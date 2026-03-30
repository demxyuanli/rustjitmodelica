export interface AnnotationPoint {
  x: number;
  y: number;
}
export interface AnnotationExtent {
  p1: AnnotationPoint;
  p2: AnnotationPoint;
}
export interface AnnotationColor {
  r: number;
  g: number;
  b: number;
}

/** Arrow type for line endpoints */
export type ArrowType = "none" | "arrow" | "filled" | "open" | "tshape" | "circle";

/** Fill pattern for shapes */
export type FillPattern =
  | "solid"
  | "horizontal"
  | "vertical"
  | "cross"
  | "diagCross"
  | "forward"
  | "backward"
  | "none";

/** Border pattern for rectangles */
export type BorderPattern = "solid" | "dashed" | "dotted" | "dotDashed";

/** Line pattern for strokes */
export type LinePattern = "solid" | "dashed" | "dotted" | "dotDashed";

/** Gradient stop for gradient fills */
export interface GradientStop {
  offset: number;
  color: AnnotationColor;
  opacity?: number;
}

/** Linear gradient specification */
export interface LinearGradient {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  stops: GradientStop[];
}

/** Radial gradient specification */
export interface RadialGradient {
  cx: number;
  cy: number;
  r: number;
  stops: GradientStop[];
}

/** Fill definition - solid color or gradient */
export type FillDefinition =
  | { type: "solid"; color: AnnotationColor }
  | { type: "linearGradient"; gradient: LinearGradient }
  | { type: "radialGradient"; gradient: RadialGradient };

export interface GraphicLine {
  type: "Line";
  points: AnnotationPoint[];
  color?: AnnotationColor;
  thickness?: number;
  pattern?: string;
  smooth?: string;
  arrow?: string[];
  arrowSize?: number;
  rotation?: number;
  origin?: AnnotationPoint;
  opacity?: number;
  mirrorX?: boolean;
  mirrorY?: boolean;
  layerHidden?: boolean;
  layerLocked?: boolean;
}

export interface GraphicRectangle {
  type: "Rectangle";
  extent?: AnnotationExtent;
  lineColor?: AnnotationColor;
  fillColor?: AnnotationColor;
  fillPattern?: string;
  fillGradient?:
    | { type: "linearGradient"; gradient: LinearGradient }
    | { type: "radialGradient"; gradient: RadialGradient };
  borderPattern?: string;
  lineThickness?: number;
  radius?: number;
  rotation?: number;
  origin?: AnnotationPoint;
  opacity?: number;
  mirrorX?: boolean;
  mirrorY?: boolean;
  layerHidden?: boolean;
  layerLocked?: boolean;
}

export interface GraphicEllipse {
  type: "Ellipse";
  extent?: AnnotationExtent;
  lineColor?: AnnotationColor;
  fillColor?: AnnotationColor;
  fillPattern?: string;
  fillGradient?:
    | { type: "linearGradient"; gradient: LinearGradient }
    | { type: "radialGradient"; gradient: RadialGradient };
  startAngle?: number;
  endAngle?: number;
  lineThickness?: number;
  linePattern?: string;
  rotation?: number;
  origin?: AnnotationPoint;
  opacity?: number;
  mirrorX?: boolean;
  mirrorY?: boolean;
  layerHidden?: boolean;
  layerLocked?: boolean;
}

export interface GraphicPolygon {
  type: "Polygon";
  points: AnnotationPoint[];
  lineColor?: AnnotationColor;
  fillColor?: AnnotationColor;
  fillPattern?: string;
  fillGradient?:
    | { type: "linearGradient"; gradient: LinearGradient }
    | { type: "radialGradient"; gradient: RadialGradient };
  lineThickness?: number;
  linePattern?: string;
  smooth?: string;
  rotation?: number;
  origin?: AnnotationPoint;
  opacity?: number;
  mirrorX?: boolean;
  mirrorY?: boolean;
  layerHidden?: boolean;
  layerLocked?: boolean;
}

export interface GraphicText {
  type: "Text";
  extent?: AnnotationExtent;
  textString?: string;
  fontSize?: number;
  fontName?: string;
  textColor?: AnnotationColor;
  lineColor?: AnnotationColor;
  fillColor?: AnnotationColor;
  horizontalAlignment?: string;
  fillPattern?: string;
  rotation?: number;
  origin?: AnnotationPoint;
  opacity?: number;
  mirrorX?: boolean;
  mirrorY?: boolean;
  layerHidden?: boolean;
  layerLocked?: boolean;
}

export interface GraphicBitmap {
  type: "Bitmap";
  extent?: AnnotationExtent;
  fileName?: string;
  imageSource?: string;
  rotation?: number;
  origin?: AnnotationPoint;
  opacity?: number;
  mirrorX?: boolean;
  mirrorY?: boolean;
  layerHidden?: boolean;
  layerLocked?: boolean;
}

/** Bezier spline curve for smooth connections */
export interface GraphicBSpline {
  type: "BSpline";
  points: AnnotationPoint[];
  color?: AnnotationColor;
  thickness?: number;
  pattern?: string;
  smooth?: string;
  arrow?: string[];
  arrowSize?: number;
  rotation?: number;
  origin?: AnnotationPoint;
  opacity?: number;
  mirrorX?: boolean;
  mirrorY?: boolean;
  layerHidden?: boolean;
  layerLocked?: boolean;
}

/** Editor-only group; children share z-order under one top-level layer row. */
export interface GraphicGroup {
  type: "Group";
  children: GraphicItem[];
  rotation?: number;
  origin?: AnnotationPoint;
  opacity?: number;
  mirrorX?: boolean;
  mirrorY?: boolean;
  layerHidden?: boolean;
  layerLocked?: boolean;
}

export type GraphicItem =
  | GraphicLine
  | GraphicRectangle
  | GraphicEllipse
  | GraphicPolygon
  | GraphicText
  | GraphicBitmap
  | GraphicBSpline
  | GraphicGroup;

export interface CoordinateSystem {
  extent?: AnnotationExtent;
  preserveAspectRatio?: boolean;
  initialScale?: number;
}

export interface IconDiagramAnnotation {
  coordinateSystem?: CoordinateSystem;
  graphics: GraphicItem[];
}

export interface LineAnnotation {
  points: AnnotationPoint[];
  color?: AnnotationColor;
  thickness?: number;
  pattern?: string;
  smooth?: string;
}

export interface GraphicBounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

export interface ConnectorAnchor {
  id: string;
  point: AnnotationPoint;
}

export const DEFAULT_ICON_SIZE = 40;

export type GraphicEditHandle =
  | { kind: "extent-corner"; cornerIndex: number }
  | { kind: "poly-point"; pointIndex: number };
