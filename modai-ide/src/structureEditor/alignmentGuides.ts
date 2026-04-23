import { ALIGN_GUIDE_THRESHOLD_PX } from "./layoutConstants";

export interface GuideSegment {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

export interface AlignmentSnapResult {
  x: number;
  y: number;
  guides: GuideSegment[];
}

export interface BoxLayout {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

function pickBestDelta(
  candidates: Array<{ delta: number; guide: GuideSegment }>,
  threshold: number,
): { delta: number; guide: GuideSegment } | null {
  let best: { delta: number; guide: GuideSegment } | null = null;
  let bestAbs = threshold + 1;
  for (const c of candidates) {
    const a = Math.abs(c.delta);
    if (a <= threshold && a < bestAbs) {
      best = c;
      bestAbs = a;
    }
  }
  return best;
}

/**
 * Snap draft top-left (draftX, draftY) against other node boxes (OMEdit-style alignment).
 */
export function snapPositionWithAlignmentGuides(
  draggingId: string,
  draftX: number,
  draftY: number,
  boxW: number,
  boxH: number,
  others: BoxLayout[],
): AlignmentSnapResult {
  const thr = ALIGN_GUIDE_THRESHOLD_PX;
  const myL = draftX;
  const myR = draftX + boxW;
  const myCx = draftX + boxW / 2;
  const myT = draftY;
  const myB = draftY + boxH;
  const myCy = draftY + boxH / 2;

  const xCandidates: Array<{ delta: number; guide: GuideSegment }> = [];
  const yCandidates: Array<{ delta: number; guide: GuideSegment }> = [];
  const ySpanPad = 8000;
  const xSpanPad = 8000;

  for (const o of others) {
    if (o.id === draggingId) continue;
    const ol = o.x;
    const or = o.x + o.width;
    const oc = o.x + o.width / 2;
    const ot = o.y;
    const ob = o.y + o.height;
    const ocy = o.y + o.height / 2;

    const y0 = Math.min(myT, ot) - ySpanPad;
    const y1 = Math.max(myB, ob) + ySpanPad;
    const x0 = Math.min(myL, ol) - xSpanPad;
    const x1 = Math.max(myR, or) + xSpanPad;

    xCandidates.push({ delta: ol - myL, guide: { x1: ol, y1: y0, x2: ol, y2: y1 } });
    xCandidates.push({ delta: or - myR, guide: { x1: or, y1: y0, x2: or, y2: y1 } });
    xCandidates.push({ delta: oc - myCx, guide: { x1: oc, y1: y0, x2: oc, y2: y1 } });
    xCandidates.push({ delta: ol - myR, guide: { x1: ol, y1: y0, x2: ol, y2: y1 } });
    xCandidates.push({ delta: or - myL, guide: { x1: or, y1: y0, x2: or, y2: y1 } });

    yCandidates.push({ delta: ot - myT, guide: { x1: x0, y1: ot, x2: x1, y2: ot } });
    yCandidates.push({ delta: ob - myB, guide: { x1: x0, y1: ob, x2: x1, y2: ob } });
    yCandidates.push({ delta: ocy - myCy, guide: { x1: x0, y1: ocy, x2: x1, y2: ocy } });
    yCandidates.push({ delta: ot - myB, guide: { x1: x0, y1: ot, x2: x1, y2: ot } });
    yCandidates.push({ delta: ob - myT, guide: { x1: x0, y1: ob, x2: x1, y2: ob } });
  }

  const bx = pickBestDelta(xCandidates, thr);
  const by = pickBestDelta(yCandidates, thr);
  const guides: GuideSegment[] = [];
  let nx = draftX;
  let ny = draftY;
  if (bx) {
    nx = draftX + bx.delta;
    guides.push(bx.guide);
  }
  if (by) {
    ny = draftY + by.delta;
    guides.push(by.guide);
  }
  return { x: nx, y: ny, guides };
}
