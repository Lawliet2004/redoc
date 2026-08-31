import { describe, expect, it } from "vitest";
import { mapReviewCommentAnchors } from "./review";
import type { ReviewComment } from "./types";

const comment: ReviewComment = {
  id: "c1",
  author: "You",
  text: "Check this",
  from: 4,
  to: 12,
  resolved: false,
  createdAt: "2026-08-31T00:00:00.000Z",
};

describe("review comment anchors", () => {
  it("maps both ends immutably after an editor transaction", () => {
    const mapped = mapReviewCommentAnchors([comment], (position) => position + 2, (position) => position + 1);
    expect(mapped[0]).toMatchObject({ from: 6, to: 13 });
    expect(comment.from).toBe(4);
  });
});
