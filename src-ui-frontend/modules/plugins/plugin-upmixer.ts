// Upmixer Plugin
// Stereo to 5.0 surround upmixer with level metering

import { BasePlugin } from './plugin-base';
import { PluginMenubar } from './plugin-menubar';
import { LevelMeter } from './level-meter';
import type { PluginMetadata, LevelMeterData } from './plugin-types';

/**
 * Channel groups for mute/solo control
 */
interface ChannelGroup {
  name: string;
  channels: number[];  // Channel indices
  muted: boolean;
  solo: boolean;
}

/**
 * Upmixer Plugin
 * Converts stereo (2ch) to 5.0 surround (L, R, C, LFE, SL, SR)
 */
export class UpmixerPlugin extends BasePlugin {
  public readonly metadata: PluginMetadata = {
    id: 'upmixer-plugin',
    name: 'SotF: Upmixer',
    category: 'spatial',
    version: '1.0.0',
  };

  // UI components
  private menubar: PluginMenubar | null = null;
  private inputMeter: LevelMeter | null = null;
  private outputMeter: LevelMeter | null = null;

  // UI elements
  private parametersContainer: HTMLElement | null = null;
  private muteButtons: Map<string, HTMLButtonElement> = new Map();
  private soloButtons: Map<string, HTMLButtonElement> = new Map();

  // Channel groups (for mute/solo)
  private channelGroups: ChannelGroup[] = [
    { name: 'L+R', channels: [0, 1], muted: false, solo: false },
    { name: 'C', channels: [2], muted: false, solo: false },
    { name: 'LFE', channels: [3], muted: false, solo: false },
    { name: 'SL+SR', channels: [4, 5], muted: false, solo: false },
  ];

  // Parameters
  private centerLevel: number = -3.0;       // Center channel level (dB)
  private surroundLevel: number = -3.0;     // Surround level (dB)
  private lfeLevel: number = 0.0;           // LFE level (dB)
  private crossfeedAmount: number = 0.5;    // Surround crossfeed (0-1)

  /**
   * Render the plugin UI
   */
  render(standalone: boolean): void {
    if (!this.container) return;

    this.container.innerHTML = `
      <div class="upmixer-plugin ${standalone ? 'standalone' : 'embedded'}">
        ${standalone ? '<div class="upmixer-menubar-container"></div>' : ''}
        <div class="upmixer-content">
          <!-- Input Meters (L, R) -->
          <div class="upmixer-input-section">
            <div class="section-label">Input</div>
            <canvas class="upmixer-input-meters" width="60" height="200"></canvas>
            <div class="meter-labels">
              <span>L</span>
              <span>R</span>
            </div>
          </div>

          <!-- Parameters -->
          <div class="upmixer-parameters"></div>

          <!-- Output Meters (L, R, C, LFE, SL, SR) -->
          <div class="upmixer-output-section">
            <div class="section-label">Output</div>
            <canvas class="upmixer-output-meters" width="150" height="200"></canvas>
            <div class="meter-labels-output">
              <span>L</span>
              <span>R</span>
              <span>C</span>
              <span>LFE</span>
              <span>SL</span>
              <span>SR</span>
            </div>
            <!-- Mute/Solo Controls -->
            <div class="upmixer-controls">
              ${this.renderMuteSoloControls()}
            </div>
          </div>
        </div>
      </div>
    `;

    // Initialize menubar if standalone
    if (standalone) {
      const menubarContainer = this.container.querySelector('.upmixer-menubar-container') as HTMLElement;
      if (menubarContainer) {
        this.menubar = new PluginMenubar(menubarContainer, this.metadata.name);
      }
    }

    // Cache elements
    this.parametersContainer = this.container.querySelector('.upmixer-parameters') as HTMLElement;

    // Initialize meters
    const inputCanvas = this.container.querySelector('.upmixer-input-meters') as HTMLCanvasElement;
    if (inputCanvas) {
      this.inputMeter = new LevelMeter({
        canvas: inputCanvas,
        channels: 2,
        channelLabels: ['L', 'R'],
      });
    }

    const outputCanvas = this.container.querySelector('.upmixer-output-meters') as HTMLCanvasElement;
    if (outputCanvas) {
      this.outputMeter = new LevelMeter({
        canvas: outputCanvas,
        channels: 6,
        channelLabels: ['L', 'R', 'C', 'LFE', 'SL', 'SR'],
      });
    }

    // Render parameters
    this.renderParameters();
    this.attachEventListeners();
  }

  /**
   * Render mute/solo controls
   */
  private renderMuteSoloControls(): string {
    return this.channelGroups.map((group, idx) => `
      <div class="control-group" data-group-index="${idx}">
        <div class="group-label">${group.name}</div>
        <button class="control-btn mute-btn ${group.muted ? 'active' : ''}" data-group-index="${idx}" title="Mute">M</button>
        <button class="control-btn solo-btn ${group.solo ? 'active' : ''}" data-group-index="${idx}" title="Solo">S</button>
      </div>
    `).join('');
  }

  /**
   * Render parameter controls
   */
  private renderParameters(): void {
    if (!this.parametersContainer) return;

    this.parametersContainer.innerHTML = `
      <div class="parameter-section">
        <h4>Spatial Processing</h4>

        <div class="parameter-group">
          <label>
            Center Level
            <span class="param-value">${this.centerLevel.toFixed(1)} dB</span>
          </label>
          <input type="range" class="param-slider" data-param="centerLevel"
                 min="-12" max="0" step="0.1" value="${this.centerLevel}" />
        </div>

        <div class="parameter-group">
          <label>
            Surround Level
            <span class="param-value">${this.surroundLevel.toFixed(1)} dB</span>
          </label>
          <input type="range" class="param-slider" data-param="surroundLevel"
                 min="-12" max="0" step="0.1" value="${this.surroundLevel}" />
        </div>

        <div class="parameter-group">
          <label>
            LFE Level
            <span class="param-value">${this.lfeLevel.toFixed(1)} dB</span>
          </label>
          <input type="range" class="param-slider" data-param="lfeLevel"
                 min="-12" max="0" step="0.1" value="${this.lfeLevel}" />
        </div>

        <div class="parameter-group">
          <label>
            Crossfeed
            <span class="param-value">${(this.crossfeedAmount * 100).toFixed(0)}%</span>
          </label>
          <input type="range" class="param-slider" data-param="crossfeedAmount"
                 min="0" max="1" step="0.01" value="${this.crossfeedAmount}" />
        </div>
      </div>
    `;

    this.attachParameterListeners();
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Mute buttons
    const muteButtons = this.container?.querySelectorAll('.mute-btn') || [];
    muteButtons.forEach((btn) => {
      const index = parseInt((btn as HTMLElement).dataset.groupIndex!, 10);
      this.muteButtons.set(`group-${index}`, btn as HTMLButtonElement);

      btn.addEventListener('click', () => {
        this.toggleMute(index);
      });
    });

    // Solo buttons
    const soloButtons = this.container?.querySelectorAll('.solo-btn') || [];
    soloButtons.forEach((btn) => {
      const index = parseInt((btn as HTMLElement).dataset.groupIndex!, 10);
      this.soloButtons.set(`group-${index}`, btn as HTMLButtonElement);

      btn.addEventListener('click', () => {
        this.toggleSolo(index);
      });
    });
  }

  /**
   * Attach parameter listeners
   */
  private attachParameterListeners(): void {
    const sliders = this.parametersContainer?.querySelectorAll('.param-slider') || [];
    sliders.forEach((slider) => {
      slider.addEventListener('input', (e) => {
        const param = (e.target as HTMLElement).dataset.param!;
        const value = parseFloat((e.target as HTMLInputElement).value);

        // Update parameter
        (this as any)[param] = value;

        // Update display
        const valueDisplay = (e.target as HTMLElement).parentElement?.querySelector('.param-value');
        if (valueDisplay) {
          if (param === 'crossfeedAmount') {
            valueDisplay.textContent = `${(value * 100).toFixed(0)}%`;
          } else {
            valueDisplay.textContent = `${value.toFixed(1)} dB`;
          }
        }

        // Notify parameter change
        this.updateParameter(param, value);
      });
    });
  }

  /**
   * Toggle mute for a channel group
   */
  private toggleMute(groupIndex: number): void {
    const group = this.channelGroups[groupIndex];
    if (!group) return;

    group.muted = !group.muted;

    // Update UI
    const btn = this.muteButtons.get(`group-${groupIndex}`);
    if (btn) {
      btn.classList.toggle('active', group.muted);
    }

    // Notify
    this.emit('groupMuteChange', { group: group.name, muted: group.muted });
  }

  /**
   * Toggle solo for a channel group
   */
  private toggleSolo(groupIndex: number): void {
    const group = this.channelGroups[groupIndex];
    if (!group) return;

    group.solo = !group.solo;

    // Update UI
    const btn = this.soloButtons.get(`group-${groupIndex}`);
    if (btn) {
      btn.classList.toggle('active', group.solo);
    }

    // Check if any group is soloed
    const anySoloed = this.channelGroups.some((g) => g.solo);

    // If solo mode is active, mute all non-soloed groups
    this.channelGroups.forEach((g, idx) => {
      if (anySoloed && !g.solo) {
        // Implicitly muted by solo
        const muteBtn = this.muteButtons.get(`group-${idx}`);
        if (muteBtn) {
          muteBtn.classList.add('implicit-mute');
        }
      } else {
        const muteBtn = this.muteButtons.get(`group-${idx}`);
        if (muteBtn) {
          muteBtn.classList.remove('implicit-mute');
        }
      }
    });

    // Notify
    this.emit('groupSoloChange', { group: group.name, solo: group.solo });
  }

  /**
   * Update input meters
   */
  updateInputMeters(data: LevelMeterData): void {
    if (this.inputMeter) {
      this.inputMeter.update(data);
    }
  }

  /**
   * Update output meters
   */
  updateOutputMeters(data: LevelMeterData): void {
    if (this.outputMeter) {
      this.outputMeter.update(data);
    }
  }

  /**
   * Get current parameters
   */
  getParameters() {
    return {
      centerLevel: this.centerLevel,
      surroundLevel: this.surroundLevel,
      lfeLevel: this.lfeLevel,
      crossfeedAmount: this.crossfeedAmount,
    };
  }

  /**
   * Set parameters
   */
  setParameters(params: Partial<{
    centerLevel: number;
    surroundLevel: number;
    lfeLevel: number;
    crossfeedAmount: number;
  }>): void {
    if (params.centerLevel !== undefined) this.centerLevel = params.centerLevel;
    if (params.surroundLevel !== undefined) this.surroundLevel = params.surroundLevel;
    if (params.lfeLevel !== undefined) this.lfeLevel = params.lfeLevel;
    if (params.crossfeedAmount !== undefined) this.crossfeedAmount = params.crossfeedAmount;

    this.renderParameters();
  }

  /**
   * Get channel groups
   */
  getChannelGroups(): ChannelGroup[] {
    return [...this.channelGroups];
  }

  /**
   * Resize handler
   */
  resize(): void {
    if (this.inputMeter) {
      this.inputMeter.resize();
    }
    if (this.outputMeter) {
      this.outputMeter.resize();
    }
  }

  /**
   * Destroy the plugin
   */
  destroy(): void {
    if (this.menubar) {
      this.menubar.destroy();
      this.menubar = null;
    }

    if (this.inputMeter) {
      this.inputMeter.destroy();
      this.inputMeter = null;
    }

    if (this.outputMeter) {
      this.outputMeter.destroy();
      this.outputMeter = null;
    }

    this.muteButtons.clear();
    this.soloButtons.clear();

    super.destroy();
  }
}
