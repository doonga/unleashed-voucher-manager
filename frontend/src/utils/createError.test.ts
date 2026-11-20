import { test } from "node:test";
import assert from "node:assert/strict";
import { createErrorMessage } from "./createError.ts";

function failure(status: number, message: string) {
  return Object.assign(new Error(message), { status });
}

test("a taken key says so", () => {
  assert.equal(createErrorMessage(failure(409, "The key key already exists.")), "That key is already in use");
});

test("a rejected request shows the backend's reason", () => {
  assert.equal(
    createErrorMessage(failure(400, "name cannot contain '''")),
    "name cannot contain '''",
  );
});

test("anything else is a generic failure", () => {
  assert.equal(createErrorMessage(failure(502, "Bad Gateway")), "Failed to create voucher");
  assert.equal(createErrorMessage(failure(400, "")), "Failed to create voucher");
  assert.equal(createErrorMessage(new Error("network")), "Failed to create voucher");
  assert.equal(createErrorMessage("weird"), "Failed to create voucher");
});
