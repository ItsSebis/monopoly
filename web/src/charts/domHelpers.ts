// Tiny DOM builders shared by singleRunCharts.ts and batchCharts.ts - not
// stats logic, just the repeated "labeled section" / "canvas" / "table"
// shapes both dashboards are made of.

export function chartSection(container: HTMLElement, title: string): HTMLElement {
  const el = document.createElement("section");
  el.className = "chart-section";
  const heading = document.createElement("h3");
  heading.textContent = title;
  el.appendChild(heading);
  container.appendChild(el);
  return el;
}

export function canvasIn(parent: HTMLElement): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  parent.appendChild(canvas);
  return canvas;
}

export function renderTable(parent: HTMLElement, headers: string[], rows: (string | number)[][]): void {
  const el = document.createElement("table");
  const headRow = document.createElement("tr");
  headers.forEach((h) => {
    const th = document.createElement("th");
    th.textContent = h;
    headRow.appendChild(th);
  });
  el.appendChild(headRow);

  if (rows.length === 0) {
    const tr = document.createElement("tr");
    const td = document.createElement("td");
    td.colSpan = headers.length;
    td.textContent = "None";
    tr.appendChild(td);
    el.appendChild(tr);
  }
  rows.forEach((row) => {
    const tr = document.createElement("tr");
    row.forEach((cell) => {
      const td = document.createElement("td");
      td.textContent = String(cell);
      tr.appendChild(td);
    });
    el.appendChild(tr);
  });
  parent.appendChild(el);
}
