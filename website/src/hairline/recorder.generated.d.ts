type FigureHandle = { set(value: number): void; setPlaying(playing: boolean): void; destroy(): void };

export const HL: {
  inject(root: Document): void;
  mk(tag: "svg", attributes: Record<string, string>, parent: HTMLElement): SVGSVGElement;
};

export const recorder: {
  name: string;
  means: string;
  range: [number, number, number];
  mount(context: {
    stage: HTMLElement;
    svg: SVGSVGElement;
    read: { textContent: string };
    onFrame?: (elapsed: number) => void;
  }, value: number): FigureHandle;
};
