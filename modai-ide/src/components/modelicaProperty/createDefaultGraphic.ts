import type {
  GraphicBSpline,
  GraphicEllipse,
  GraphicItem,
  GraphicLine,
  GraphicPolygon,
  GraphicRectangle,
  GraphicText,
} from "../diagramGraphicTypes";

export function createDefaultGraphic(kind: GraphicItem["type"]): GraphicItem {
  switch (kind) {
    case "Rectangle":
      return {
        type: "Rectangle",
        extent: { p1: { x: -60, y: 40 }, p2: { x: 60, y: -40 } },
        lineColor: { r: 0, g: 0, b: 255 },
      } satisfies GraphicRectangle;
    case "Ellipse":
      return {
        type: "Ellipse",
        extent: { p1: { x: -50, y: 50 }, p2: { x: 50, y: -50 } },
        lineColor: { r: 191, g: 0, b: 0 },
      } satisfies GraphicEllipse;
    case "Line":
      return {
        type: "Line",
        points: [{ x: -80, y: 0 }, { x: 80, y: 0 }],
        color: { r: 0, g: 127, b: 0 },
      } satisfies GraphicLine;
    case "Polygon":
      return {
        type: "Polygon",
        points: [{ x: -60, y: -40 }, { x: 0, y: 60 }, { x: 60, y: -40 }],
        lineColor: { r: 85, g: 85, b: 85 },
      } satisfies GraphicPolygon;
    case "Text":
      return {
        type: "Text",
        extent: { p1: { x: -100, y: 90 }, p2: { x: 100, y: 50 } },
        textString: "%name",
        textColor: { r: 0, g: 0, b: 255 },
      } satisfies GraphicText;
    case "Bitmap":
      return {
        type: "Bitmap",
        extent: { p1: { x: -50, y: 50 }, p2: { x: 50, y: -50 } },
        fileName: "",
      };
    case "BSpline":
      return {
        type: "BSpline",
        points: [{ x: -80, y: -20 }, { x: -40, y: 20 }, { x: 0, y: -20 }, { x: 40, y: 20 }, { x: 80, y: -20 }],
        color: { r: 128, g: 0, b: 128 },
        smooth: "BSpline",
      } satisfies GraphicBSpline;
    default:
      return {
        type: "Rectangle",
        extent: { p1: { x: -40, y: 40 }, p2: { x: 40, y: -40 } },
      } satisfies GraphicRectangle;
  }
}
