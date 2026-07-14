export const utf8Len = (s: string): number => new TextEncoder().encode(s).length;

export const MAX_TITLE_BYTES = 66;
