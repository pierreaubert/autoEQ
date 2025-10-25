/**
 * Audio channel routing configuration module
 * Provides UI and logic for configuring channel routing matrices
 */

export interface ChannelInfo {
  index: number;
  name: string;
}

export interface RoutingConfig {
  channelCount: number;
  routing: number[]; // routing[i] = j means logical channel i is routed to physical channel j
}

const CHANNEL_NAMES = [
  "Left",
  "Right",
  "Center",
  "Subwoofer",
  "SR", // Surround Right
  "SL", // Surround Left
  "RR", // Rear Right
  "RL", // Rear Left
];

/**
 * Get channel name for a given index
 */
export function getChannelName(index: number, totalChannels: number): string {
  if (index < CHANNEL_NAMES.length) {
    return CHANNEL_NAMES[index];
  }
  return `Channel ${index + 1}`;
}

/**
 * Create default identity routing (channel i -> channel i)
 */
export function createDefaultRouting(channelCount: number): number[] {
  return Array.from({ length: channelCount }, (_, i) => i);
}

/**
 * Routing matrix manager class
 */
export class RoutingMatrix {
  private channelCount: number;
  private routing: number[];
  private container: HTMLElement | null = null;
  private overlay: HTMLElement | null = null;
  private onRoutingChange?: (routing: number[]) => void;

  constructor(channelCount: number, initialRouting?: number[]) {
    this.channelCount = channelCount;
    this.routing = initialRouting || createDefaultRouting(channelCount);
  }

  /**
   * Set callback for when routing changes
   */
  public setOnRoutingChange(callback: (routing: number[]) => void): void {
    this.onRoutingChange = callback;
  }

  /**
   * Get current routing configuration
   */
  public getRouting(): number[] {
    return [...this.routing];
  }

  /**
   * Set routing configuration
   */
  public setRouting(routing: number[]): void {
    if (routing.length !== this.channelCount) {
      throw new Error("Routing array length must match channel count");
    }
    this.routing = [...routing];
  }

  /**
   * Update channel count and reset routing if needed
   */
  public updateChannelCount(channelCount: number): void {
    if (this.channelCount !== channelCount) {
      this.channelCount = channelCount;
      this.routing = createDefaultRouting(channelCount);
      if (this.onRoutingChange) {
        this.onRoutingChange(this.routing);
      }
    }
  }

  /**
   * Show the routing matrix UI
   */
  public show(anchorElement: HTMLElement): void {
    // Create overlay
    this.overlay = document.createElement("div");
    this.overlay.className = "routing-overlay";
    this.overlay.addEventListener("click", (e) => {
      if (e.target === this.overlay) {
        this.hide();
      }
    });

    // Create container
    this.container = document.createElement("div");
    this.container.className = "routing-matrix-container";

    // Position near anchor
    const rect = anchorElement.getBoundingClientRect();
    this.container.style.top = `${rect.bottom + 5}px`;
    this.container.style.left = `${rect.left}px`;

    // Build matrix UI
    this.buildMatrixUI();

    this.overlay.appendChild(this.container);
    document.body.appendChild(this.overlay);

    // Add escape key handler
    const escapeHandler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        this.hide();
        document.removeEventListener("keydown", escapeHandler);
      }
    };
    document.addEventListener("keydown", escapeHandler);
  }

  /**
   * Hide the routing matrix UI
   */
  public hide(): void {
    if (this.overlay) {
      this.overlay.remove();
      this.overlay = null;
    }
    this.container = null;
  }

  /**
   * Build the matrix UI
   */
  private buildMatrixUI(): void {
    if (!this.container) return;

    // Clear container
    this.container.innerHTML = "";

    // Title
    const title = document.createElement("div");
    title.className = "routing-matrix-title";
    title.textContent = "Channel Routing";
    this.container.appendChild(title);

    // Set grid template columns dynamically for all rows
    const gridTemplate = `80px repeat(${this.channelCount}, 32px)`;

    // Create header row with physical channel numbers
    const headerRow = document.createElement("div");
    headerRow.className = "routing-matrix-header";
    headerRow.style.gridTemplateColumns = gridTemplate;

    // Empty cell for row labels
    const emptyCell = document.createElement("div");
    emptyCell.className = "routing-matrix-cell routing-matrix-corner";
    headerRow.appendChild(emptyCell);

    // Physical channel headers
    for (let i = 0; i < this.channelCount; i++) {
      const headerCell = document.createElement("div");
      headerCell.className = "routing-matrix-cell routing-matrix-header-cell";
      headerCell.textContent = `${i + 1}`;
      headerCell.title = `Physical Channel ${i + 1}`;
      headerRow.appendChild(headerCell);
    }
    this.container.appendChild(headerRow);

    // Create rows for each logical channel
    for (
      let logicalChannel = 0;
      logicalChannel < this.channelCount;
      logicalChannel++
    ) {
      const row = document.createElement("div");
      row.className = "routing-matrix-row";
      row.style.gridTemplateColumns = gridTemplate;

      // Row label (logical channel name)
      const labelCell = document.createElement("div");
      labelCell.className = "routing-matrix-cell routing-matrix-label-cell";
      labelCell.textContent = getChannelName(logicalChannel, this.channelCount);
      labelCell.title = `Logical: ${getChannelName(logicalChannel, this.channelCount)}`;
      row.appendChild(labelCell);

      // Create cells for each physical channel
      for (
        let physicalChannel = 0;
        physicalChannel < this.channelCount;
        physicalChannel++
      ) {
        const cell = document.createElement("div");
        cell.className = "routing-matrix-cell routing-matrix-data-cell";

        // Check if this is the current routing
        const isActive = this.routing[logicalChannel] === physicalChannel;
        if (isActive) {
          cell.classList.add("routing-active");
          cell.textContent = "×";
        }

        // Click handler
        cell.addEventListener("click", () => {
          this.handleCellClick(logicalChannel, physicalChannel);
        });

        row.appendChild(cell);
      }

      this.container.appendChild(row);
    }

    // Close button
    const closeButton = document.createElement("button");
    closeButton.className = "routing-matrix-close";
    closeButton.textContent = "Close";
    closeButton.addEventListener("click", () => this.hide());
    this.container.appendChild(closeButton);
  }

  /**
   * Handle cell click - swap routing
   */
  private handleCellClick(
    logicalChannel: number,
    physicalChannel: number,
  ): void {
    // Find if any other logical channel is routed to this physical channel
    const otherLogicalChannel = this.routing.indexOf(physicalChannel);

    if (otherLogicalChannel !== -1 && otherLogicalChannel !== logicalChannel) {
      // Swap: exchange routing between the two logical channels
      const temp = this.routing[logicalChannel];
      this.routing[logicalChannel] = physicalChannel;
      this.routing[otherLogicalChannel] = temp;
    } else {
      // Direct assignment (shouldn't normally happen, but handle it)
      this.routing[logicalChannel] = physicalChannel;
    }

    // Notify change
    if (this.onRoutingChange) {
      this.onRoutingChange(this.routing);
    }

    // Rebuild UI to reflect changes
    this.buildMatrixUI();
  }
}

/**
 * Create a routing button element with mini matrix icon
 */
export function createRoutingButton(): HTMLButtonElement {
  const button = document.createElement("button");
  button.className = "routing-button";
  button.title = "Configure channel routing";
  button.setAttribute("aria-label", "Configure channel routing");

  // Create mini matrix SVG icon
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "16");
  svg.setAttribute("height", "16");
  svg.setAttribute("viewBox", "0 0 16 16");
  svg.setAttribute("fill", "currentColor");

  // Draw a simple 3x3 grid with one diagonal marked
  const gridSize = 3;
  const cellSize = 16 / gridSize;

  // Draw grid lines
  for (let i = 0; i <= gridSize; i++) {
    // Vertical lines
    const vLine = document.createElementNS(
      "http://www.w3.org/2000/svg",
      "line",
    );
    vLine.setAttribute("x1", `${i * cellSize}`);
    vLine.setAttribute("y1", "0");
    vLine.setAttribute("x2", `${i * cellSize}`);
    vLine.setAttribute("y2", "16");
    vLine.setAttribute("stroke", "currentColor");
    vLine.setAttribute("stroke-width", "1");
    svg.appendChild(vLine);

    // Horizontal lines
    const hLine = document.createElementNS(
      "http://www.w3.org/2000/svg",
      "line",
    );
    hLine.setAttribute("x1", "0");
    hLine.setAttribute("y1", `${i * cellSize}`);
    hLine.setAttribute("x2", "16");
    hLine.setAttribute("y2", `${i * cellSize}`);
    hLine.setAttribute("stroke", "currentColor");
    hLine.setAttribute("stroke-width", "1");
    svg.appendChild(hLine);
  }

  // Draw diagonal marks (identity routing) in green
  for (let i = 0; i < gridSize; i++) {
    const circle = document.createElementNS(
      "http://www.w3.org/2000/svg",
      "circle",
    );
    circle.setAttribute("cx", `${i * cellSize + cellSize / 2}`);
    circle.setAttribute("cy", `${i * cellSize + cellSize / 2}`);
    circle.setAttribute("r", "1.5");
    circle.setAttribute("fill", "#57F287"); // Green color
    svg.appendChild(circle);
  }

  button.appendChild(svg);
  return button;
}

/**
 * Initialize routing for a device section
 */
export function initializeRouting(
  buttonElement: HTMLElement,
  channelCount: number,
  onRoutingChange: (routing: number[]) => void,
): RoutingMatrix {
  const matrix = new RoutingMatrix(channelCount);
  matrix.setOnRoutingChange(onRoutingChange);

  buttonElement.addEventListener("click", () => {
    matrix.show(buttonElement);
  });

  return matrix;
}
