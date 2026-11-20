import { test } from "node:test";
import assert from "node:assert/strict";
import { keyError, nameError, remarksError } from "./validation.ts";

test("names reject what the controller refuses", () => {
  for (const c of ["'", '"', "<", ">", "&", "#", ";", "\\", "`", "|", "!", "$", "(", ")"]) {
    const error = nameError(`a${c}b`);
    assert.ok(error && error.includes(c), `${c}: ${error}`);
  }
});

test("names allow dashes brackets dots and spaces", () => {
  assert.equal(nameError("[ROLLING]-1"), null);
  assert.equal(nameError("front.desk-2"), null);
  assert.equal(nameError("Quick Voucher"), null);
});

test("remarks allow apostrophes and commas", () => {
  assert.equal(remarksError("Bob's phone, room 4"), null);
  assert.equal(remarksError(""), null);
});

test("remarks reject angle brackets", () => {
  assert.ok(remarksError("<b>")?.includes("<"));
  assert.ok(remarksError("a > b"));
});

test("non-ASCII is rejected", () => {
  assert.ok(nameError("café"));
  assert.ok(remarksError("café"));
});

test("keys follow the length and character rules", () => {
  assert.equal(keyError(""), null);
  assert.equal(keyError("ab"), null);
  assert.equal(keyError("ABC123"), null);
  for (const bad of ["a", "a234567890123456b", "a b", "a#b", "a,b", "a&b", "a+b", "a'b"]) {
    assert.ok(keyError(bad), bad);
  }
});
