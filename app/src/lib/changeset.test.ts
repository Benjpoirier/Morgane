import { describe, expect, it } from "vitest";
import { buildChangeset } from "./changeset";
import type { SyncStateSnapshot } from "./ipc";
import type {
  EditDetail,
  Episode,
  PendingGroup,
  PlaylistFolder,
  PlaylistNode,
  SyncedRecord,
} from "./types";

const ep = (guid: string, title = guid): Episode => ({
  guid,
  title,
  audioUrl: "a",
  imageUrl: null,
  publishedAt: null,
  duration: null,
});

const group = (over: Partial<PendingGroup> = {}): PendingGroup => ({
  feedUrl: "https://feed",
  groupKey: "",
  uuid: "group-uuid",
  title: "Podcast",
  feedImageUrl: null,
  episodes: [ep("g1"), ep("g2")],
  ...over,
});

const edit = (over: Partial<EditDetail> & Pick<EditDetail, "kind" | "uuid">): EditDetail => ({
  title: "",
  newTitle: null,
  destTitle: null,
  ...over,
});

const folder = (uuid: string, title: string, children: PlaylistNode[] = []): PlaylistFolder => ({
  uuid,
  title,
  children,
  isFavorite: false,
  isSynthetic: false,
});

const record = (uuid: string, pendingDeletion: boolean, title = uuid): SyncedRecord => ({
  episodeUuid: uuid,
  title,
  folderTitle: "F",
  syncedAt: "",
  pendingDeletion,
});

const emptyState = (over: Partial<SyncStateSnapshot> = {}): SyncStateSnapshot => ({
  syncedRecords: [],
  episodeTitleOverrides: {},
  episodeNumberOverrides: {},
  groupTitleOverrides: {},
  categoryAssignments: {},
  episodeImageOverrides: {},
  folderImageOverrides: {},
  manualCategories: [],
  ...over,
});

const build = (
  groups: PendingGroup[] = [],
  state: SyncStateSnapshot | null = emptyState(),
  edits: EditDetail[] = [],
  orphans: string[] = [],
  folders: PlaylistFolder[] = [],
) => buildChangeset(groups, state, edits, orphans, folders);

describe("buildChangeset — vide", () => {
  it("tout vide → aucun changement", () => {
    const cs = build();
    expect(cs.total).toBe(0);
    expect(cs.additions).toEqual([]);
    expect(cs.modifications).toEqual([]);
    expect(cs.deletions).toEqual([]);
  });

  it("syncState null ne casse pas", () => {
    const cs = build([group()], null, [], [], []);
    expect(cs.additions).toHaveLength(1);
    expect(cs.additions[0].targetCategoryTitle).toBeNull();
  });
});

describe("À ajouter", () => {
  it("groupe sans affectation → non placé, pas de badge", () => {
    const a = build([group()]).additions[0];
    expect(a.targetCategoryTitle).toBeNull();
    expect(a.renamed).toBe(false);
    expect(a.hasImage).toBe(false);
    expect(a.undo).toEqual({ kind: "deselectGroup", feedUrl: "https://feed", guids: ["g1", "g2"] });
  });

  it("groupe affecté → titre de catégorie cible", () => {
    const state = emptyState({
      categoryAssignments: {
        "https://feed|": {
          feedUrl: "https://feed",
          groupKey: "",
          targetCategoryUuid: "cat-1",
          targetCategoryTitle: "Histoires",
        },
      },
    });
    expect(build([group()], state).additions[0].targetCategoryTitle).toBe("Histoires");
  });

  it("override de titre de groupe → badge renommé", () => {
    const state = emptyState({ groupTitleOverrides: { "https://feed|": "Nouveau" } });
    expect(build([group()], state).additions[0].renamed).toBe(true);
  });

  it("override d'image sur l'uuid du groupe → badge image (pas une modification)", () => {
    const state = emptyState({ folderImageOverrides: { "group-uuid": "img" } });
    const cs = build([group()], state);
    expect(cs.additions[0].hasImage).toBe(true);
    expect(cs.modifications).toEqual([]);
  });

  it("groupKey non vide → clé combinée correcte", () => {
    const g = group({ groupKey: "Saison 1", uuid: "g-s1" });
    const state = emptyState({ groupTitleOverrides: { "https://feed|Saison 1": "T" } });
    expect(build([g], state).additions[0].renamed).toBe(true);
  });

  it("plusieurs groupes → un ajout chacun", () => {
    expect(build([group({ uuid: "a" }), group({ uuid: "b", groupKey: "k2" })]).additions).toHaveLength(2);
  });
});

describe("Modifier — contenu déjà sur la Merlin", () => {
  it("renommage de dossier → ancien → nouveau", () => {
    const cs = build([], emptyState(), [
      edit({ kind: "renamedFolder", uuid: "f1", title: "Ancien", newTitle: "Détente" }),
    ]);
    expect(cs.modifications).toEqual([
      {
        id: "renamedFolder:f1",
        label: "« Ancien » → « Détente »",
        detail: "dossier",
        undo: { kind: "cancelEdit", uuid: "f1", editType: "renamedFolder" },
      },
    ]);
  });

  it("renommage de son → detail épisode", () => {
    const cs = build([], emptyState(), [
      edit({ kind: "renamedSound", uuid: "s1", title: "Vieux", newTitle: "Neuf" }),
    ]);
    expect(cs.modifications[0].label).toBe("« Vieux » → « Neuf »");
    expect(cs.modifications[0].detail).toBe("épisode");
    expect(cs.modifications[0].undo).toEqual({ kind: "cancelEdit", uuid: "s1", editType: "renamedSound" });
  });

  it("déplacement → titre du nœud + destination résolus (fournis par le backend)", () => {
    const cs = build([], emptyState(), [
      edit({ kind: "moved", uuid: "s1", title: "Mon episode", destTitle: "Calme" }),
    ]);
    expect(cs.modifications[0].label).toBe("Mon episode");
    expect(cs.modifications[0].detail).toBe("→ Calme");
  });

  it("déplacement avec titres manquants → libellés de repli", () => {
    const cs = build([], emptyState(), [edit({ kind: "moved", uuid: "x", title: "", destTitle: null })]);
    expect(cs.modifications[0].label).toBe("Élément déplacé");
    expect(cs.modifications[0].detail).toBe("→ …");
  });

  it("override d'image sur un dossier existant → modification", () => {
    const folders = [folder("cat-1", "Histoires")];
    const state = emptyState({ folderImageOverrides: { "cat-1": "img" } });
    expect(build([], state, [], [], folders).modifications).toEqual([
      { id: "folder-image:cat-1", label: "Visuel changé — Histoires", undo: { kind: "clearFolderImage", uuid: "cat-1" } },
    ]);
  });

  it("override d'image sur un uuid absent de l'arbre → ignoré", () => {
    const state = emptyState({ folderImageOverrides: { fantome: "img" } });
    expect(build([], state).modifications).toEqual([]);
  });

  it("override d'image sur un dossier SYNTHÉTIQUE → ignoré", () => {
    const synth: PlaylistFolder = { ...folder("orphans", "Fichiers retrouvés"), isSynthetic: true };
    const state = emptyState({ folderImageOverrides: { orphans: "img" } });
    expect(build([], state, [], [], [synth]).modifications).toEqual([]);
  });
});

describe("Supprimer — contenu déjà sur la Merlin", () => {
  it("épisode synchronisé marqué → suppression", () => {
    const state = emptyState({ syncedRecords: [record("u1", true, "Histoire du soir")] });
    expect(build([], state).deletions).toEqual([
      { id: "del-episode:u1", label: "Histoire du soir", detail: "épisode", undo: { kind: "unmarkDeletion", episodeUuid: "u1" } },
    ]);
  });

  it("épisode synchronisé NON marqué → pas dans les suppressions", () => {
    expect(build([], emptyState({ syncedRecords: [record("u1", false)] })).deletions).toEqual([]);
  });

  it("nœud retiré → titre fourni par le backend", () => {
    const cs = build([], emptyState(), [edit({ kind: "removed", uuid: "s1", title: "Titre connu" })]);
    expect(cs.deletions[0]).toEqual({
      id: "removed:s1",
      label: "Titre connu",
      detail: "retiré",
      undo: { kind: "cancelEdit", uuid: "s1", editType: "removed" },
    });
  });

  it("nœud retiré sans titre → libellé générique", () => {
    const cs = build([], emptyState(), [edit({ kind: "removed", uuid: "inconnu", title: "" })]);
    expect(cs.deletions[0].label).toBe("Élément retiré du menu");
  });

  it("orphelin → suppression par nom de fichier", () => {
    expect(build([], emptyState(), [], ["abc.mp3", "def.jpg"]).deletions).toEqual([
      { id: "orphan:abc.mp3", label: "abc.mp3", detail: "orphelin", undo: { kind: "toggleOrphan", name: "abc.mp3" } },
      { id: "orphan:def.jpg", label: "def.jpg", detail: "orphelin", undo: { kind: "toggleOrphan", name: "def.jpg" } },
    ]);
  });
});

describe("Cas mixtes et tordus", () => {
  it("total = somme des trois sections", () => {
    const state = emptyState({
      syncedRecords: [record("u1", true), record("u2", false)],
      folderImageOverrides: { "cat-1": "img" },
    });
    const cs = build(
      [group()],
      state,
      [edit({ kind: "renamedFolder", uuid: "cat-1", title: "A", newTitle: "B" }), edit({ kind: "removed", uuid: "u2", title: "E2" })],
      ["o.mp3"],
      [folder("cat-1", "Histoires")],
    );
    expect(cs.additions).toHaveLength(1);
    expect(cs.modifications).toHaveLength(2);
    expect(cs.deletions).toHaveLength(3);
    expect(cs.total).toBe(cs.additions.length + cs.modifications.length + cs.deletions.length);
  });

  it("uuid à la fois image + arbre + ajout → reste un ajout (badge), jamais une modif", () => {
    const g = group({ uuid: "dup" });
    const cs = build([g], emptyState({ folderImageOverrides: { dup: "img" } }), [], [], [folder("dup", "Existe")]);
    expect(cs.additions[0].hasImage).toBe(true);
    expect(cs.modifications).toEqual([]);
  });

  it("suppressions de natures différentes coexistent dans l'ordre", () => {
    const state = emptyState({ syncedRecords: [record("u1", true, "E1")] });
    const cs = build([], state, [edit({ kind: "removed", uuid: "n1", title: "N1" })], ["orph.aac"]);
    expect(cs.deletions.map((d) => d.detail)).toEqual(["épisode", "retiré", "orphelin"]);
  });

  it("ids stables et uniques", () => {
    const state = emptyState({ syncedRecords: [record("u1", true), record("u2", true)] });
    const ids = build([], state).deletions.map((d) => d.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});
