// The controller refuses these characters in a voucher name (it answers OK
// and creates nothing), in remarks, and in custom keys. These mirror the
// backend's checks so staff see the problem before submitting.
const NAME_FORBIDDEN = ["'", '"', "<", ">", "&", "#", ";", "\\", "`", "|", "!", "$", "(", ")"];
const REMARKS_FORBIDDEN = ["<", ">"];
const KEY_FORBIDDEN = ["#", "&", "+", '"', "'", "<", ">", ","];

export const MIN_KEY_LENGTH = 2;
export const MAX_KEY_LENGTH = 16;

function nonAscii(value: string): string | undefined {
  return [...value].find((c) => c < " " || c > "~");
}

function forbidden(value: string, characters: string[]): string | undefined {
  return [...value].find((c) => characters.includes(c));
}

function characterError(field: string, value: string, characters: string[]): string | null {
  const bad = nonAscii(value) ?? forbidden(value, characters);
  return bad === undefined ? null : `${field} cannot contain "${bad}"`;
}

/** Spaces are allowed: the backend turns them into dashes. */
export function nameError(name: string): string | null {
  return characterError("Name", name.replace(/\s/g, "-"), NAME_FORBIDDEN);
}

export function remarksError(remarks: string): string | null {
  return characterError("Remarks", remarks, REMARKS_FORBIDDEN);
}

/** An empty key is fine: the controller generates one. */
export function keyError(key: string): string | null {
  if (key === "") return null;
  if (key.length < MIN_KEY_LENGTH || key.length > MAX_KEY_LENGTH) {
    return `Key must be ${MIN_KEY_LENGTH} to ${MAX_KEY_LENGTH} characters`;
  }
  if (/\s/.test(key)) return "Key cannot contain spaces";
  return characterError("Key", key, KEY_FORBIDDEN);
}
