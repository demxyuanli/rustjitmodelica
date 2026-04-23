import { describe, expect, it } from "vitest";
import { snapPointToGridStrict } from "./gridSnap";

describe("snapPointToGridStrict", () => {
  it("snaps to nearest grid intersection", () => {
    expect(snapPointToGridStrict({ x: 12, y: 18 }, 10)).toEqual({ x: 10, y: 20 });
  });

  it("returns original when gridSize is non-positive", () => {
    const p = { x: 3, y: 4 };
    expect(snapPointToGridStrict(p, 0)).toEqual(p);
  });
});
