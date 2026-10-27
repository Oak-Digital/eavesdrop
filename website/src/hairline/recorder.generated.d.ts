type TourStop = [number, number] | null;
type FigureHandle = { set(value: number): void; destroy(): void };

export const HL: {
  inject(root: Document): void;
  mk(tag: "svg", attributes: Record<string, string>, parent: HTMLElement): SVGSVGElement;
  tour(stage: HTMLElement, stops: TourStop[]): { stop(): void };
};

export const recorder: {
  name: string;
  means: string;
  range: [number, number, number];
  tour: TourStop[];
  mount(context: {
    stage: HTMLElement;
    svg: SVGSVGElement;
    read: { textContent: string };
  }, value: number): FigureHandle;
};
