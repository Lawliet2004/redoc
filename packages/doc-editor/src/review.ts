import type { ReviewComment } from "./types";

export function mapReviewCommentAnchors(
  comments: readonly ReviewComment[],
  mapFrom: (position: number) => number,
  mapTo: (position: number) => number,
): ReviewComment[] {
  return comments.map((comment) => ({
    ...comment,
    from: mapFrom(comment.from),
    to: mapTo(comment.to),
  }));
}
