// Plugin Menubar Component
// Shared menubar for plugins: Name, Presets, Matrix, Mute/Solo

import './plugin-menubar.css';
import type { MenubarConfig, MenubarButton, PluginPreset } from './plugin-types';

export interface PluginMenubarCallbacks {
  onPresetLoad?: (preset: PluginPreset) => void;
  onPresetSave?: (name: string) => void;
  onMatrix?: () => void;
  onMute?: (muted: boolean) => void;
  onSolo?: (solo: boolean) => void;
  onBypass?: (bypassed: boolean) => void;
}

/**
 * Plugin Menubar Component
 * Provides consistent menubar across all plugins
 */
export class PluginMenubar {
  private container: HTMLElement;
  private config: MenubarConfig;
  private callbacks: PluginMenubarCallbacks;
  private pluginName: string;

  // State
  private presets: PluginPreset[] = [];
  private currentPresetName: string | null = null;
  private muted: boolean = false;
  private solo: boolean = false;
  private bypassed: boolean = false;

  // UI elements
  private muteButton: HTMLButtonElement | null = null;
  private soloButton: HTMLButtonElement | null = null;
  private bypassButton: HTMLButtonElement | null = null;

  constructor(
    container: HTMLElement,
    pluginName: string,
    config: MenubarConfig = {},
    callbacks: PluginMenubarCallbacks = {}
  ) {
    this.container = container;
    this.pluginName = pluginName;
    this.config = {
      showName: config.showName ?? true,
      showPresets: config.showPresets ?? true,
      showMatrix: config.showMatrix ?? true,
      showMuteSolo: config.showMuteSolo ?? true,
      customButtons: config.customButtons ?? [],
    };
    this.callbacks = callbacks;

    this.render();
  }

  /**
   * Render the menubar
   */
  render(): void {
    this.container.innerHTML = `
      <div class="plugin-menubar">
        <div class="menubar-left">
          ${this.config.showName ? `<span class="plugin-name">${this.pluginName}</span>` : ''}
        </div>
        <div class="menubar-right">
          ${this.renderPresets()}
          ${this.renderMatrix()}
          ${this.renderMuteSolo()}
          ${this.renderBypass()}
          ${this.renderCustomButtons()}
        </div>
      </div>
    `;

    this.attachEventListeners();
  }

  /**
   * Render presets dropdown
   */
  private renderPresets(): string {
    if (!this.config.showPresets) return '';

    return `
      <div class="menubar-item menubar-presets">
        <button class="menubar-button preset-button" title="Presets">
          <span>${this.currentPresetName || 'Presets'}</span>
          <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor">
            <path d="M6 8L2 4h8z"/>
          </svg>
        </button>
        <div class="preset-dropdown" style="display: none;">
          <div class="preset-list"></div>
          <div class="preset-actions">
            <button class="preset-save-btn">Save Preset</button>
          </div>
        </div>
      </div>
    `;
  }

  /**
   * Render matrix button
   */
  private renderMatrix(): string {
    if (!this.config.showMatrix) return '';

    return `
      <div class="menubar-item">
        <button class="menubar-button matrix-button" title="Routing Matrix">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
            <rect x="2" y="2" width="12" height="12" rx="1"/>
            <line x1="5" y1="2" x2="5" y2="14"/>
            <line x1="11" y1="2" x2="11" y2="14"/>
            <line x1="2" y1="5" x2="14" y2="5"/>
            <line x1="2" y1="11" x2="14" y2="11"/>
          </svg>
        </button>
      </div>
    `;
  }

  /**
   * Render mute/solo buttons
   */
  private renderMuteSolo(): string {
    if (!this.config.showMuteSolo) return '';

    return `
      <div class="menubar-item menubar-mute-solo">
        <button class="menubar-button mute-button ${this.muted ? 'active' : ''}" title="Mute">M</button>
        <button class="menubar-button solo-button ${this.solo ? 'active' : ''}" title="Solo">S</button>
      </div>
    `;
  }

  /**
   * Render bypass button
   */
  private renderBypass(): string {
    return `
      <div class="menubar-item">
        <button class="menubar-button bypass-button ${this.bypassed ? 'active' : ''}" title="Bypass">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="8" cy="8" r="6"/>
            <line x1="4" y1="12" x2="12" y2="4"/>
          </svg>
        </button>
      </div>
    `;
  }

  /**
   * Render custom buttons
   */
  private renderCustomButtons(): string {
    return this.config.customButtons!
      .map(
        (btn) => `
        <div class="menubar-item">
          <button class="menubar-button custom-button" data-button-id="${btn.id}" title="${btn.label}">
            ${btn.icon || btn.label}
          </button>
        </div>
      `
      )
      .join('');
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Preset button
    const presetButton = this.container.querySelector('.preset-button') as HTMLButtonElement;
    const presetDropdown = this.container.querySelector('.preset-dropdown') as HTMLElement;
    if (presetButton && presetDropdown) {
      presetButton.addEventListener('click', () => {
        const isVisible = presetDropdown.style.display !== 'none';
        presetDropdown.style.display = isVisible ? 'none' : 'block';
        this.updatePresetList();
      });
    }

    // Preset save button
    const presetSaveBtn = this.container.querySelector('.preset-save-btn') as HTMLButtonElement;
    if (presetSaveBtn) {
      presetSaveBtn.addEventListener('click', () => {
        const name = prompt('Enter preset name:');
        if (name && this.callbacks.onPresetSave) {
          this.callbacks.onPresetSave(name);
        }
      });
    }

    // Matrix button
    const matrixButton = this.container.querySelector('.matrix-button') as HTMLButtonElement;
    if (matrixButton) {
      matrixButton.addEventListener('click', () => {
        if (this.callbacks.onMatrix) {
          this.callbacks.onMatrix();
        }
      });
    }

    // Mute button
    this.muteButton = this.container.querySelector('.mute-button') as HTMLButtonElement;
    if (this.muteButton) {
      this.muteButton.addEventListener('click', () => {
        this.muted = !this.muted;
        this.muteButton!.classList.toggle('active', this.muted);
        if (this.callbacks.onMute) {
          this.callbacks.onMute(this.muted);
        }
      });
    }

    // Solo button
    this.soloButton = this.container.querySelector('.solo-button') as HTMLButtonElement;
    if (this.soloButton) {
      this.soloButton.addEventListener('click', () => {
        this.solo = !this.solo;
        this.soloButton!.classList.toggle('active', this.solo);
        if (this.callbacks.onSolo) {
          this.callbacks.onSolo(this.solo);
        }
      });
    }

    // Bypass button
    this.bypassButton = this.container.querySelector('.bypass-button') as HTMLButtonElement;
    if (this.bypassButton) {
      this.bypassButton.addEventListener('click', () => {
        this.bypassed = !this.bypassed;
        this.bypassButton!.classList.toggle('active', this.bypassed);
        if (this.callbacks.onBypass) {
          this.callbacks.onBypass(this.bypassed);
        }
      });
    }

    // Custom buttons
    const customButtons = this.container.querySelectorAll('.custom-button');
    customButtons.forEach((btn) => {
      const buttonId = (btn as HTMLElement).dataset.buttonId!;
      const customButton = this.config.customButtons!.find((b) => b.id === buttonId);
      if (customButton) {
        btn.addEventListener('click', () => customButton.onClick());
      }
    });

    // Close dropdown when clicking outside
    document.addEventListener('click', (e) => {
      if (presetDropdown && !this.container.contains(e.target as Node)) {
        presetDropdown.style.display = 'none';
      }
    });
  }

  /**
   * Update preset list
   */
  private updatePresetList(): void {
    const presetList = this.container.querySelector('.preset-list') as HTMLElement;
    if (!presetList) return;

    if (this.presets.length === 0) {
      presetList.innerHTML = '<div class="preset-empty">No presets available</div>';
      return;
    }

    presetList.innerHTML = this.presets
      .map(
        (preset) => `
        <button class="preset-item ${preset.name === this.currentPresetName ? 'active' : ''}" data-preset-name="${preset.name}">
          ${preset.name}
        </button>
      `
      )
      .join('');

    // Attach click handlers
    const presetItems = presetList.querySelectorAll('.preset-item');
    presetItems.forEach((item) => {
      item.addEventListener('click', () => {
        const presetName = (item as HTMLElement).dataset.presetName!;
        const preset = this.presets.find((p) => p.name === presetName);
        if (preset && this.callbacks.onPresetLoad) {
          this.callbacks.onPresetLoad(preset);
          this.setCurrentPreset(presetName);
        }
      });
    });
  }

  /**
   * Set available presets
   */
  setPresets(presets: PluginPreset[]): void {
    this.presets = presets;
    this.updatePresetList();
  }

  /**
   * Add a preset
   */
  addPreset(preset: PluginPreset): void {
    this.presets.push(preset);
    this.updatePresetList();
  }

  /**
   * Set current preset
   */
  setCurrentPreset(name: string | null): void {
    this.currentPresetName = name;
    const presetButton = this.container.querySelector('.preset-button span') as HTMLSpanElement;
    if (presetButton) {
      presetButton.textContent = name || 'Presets';
    }
    this.updatePresetList();
  }

  /**
   * Set mute state
   */
  setMuted(muted: boolean): void {
    this.muted = muted;
    if (this.muteButton) {
      this.muteButton.classList.toggle('active', muted);
    }
  }

  /**
   * Set solo state
   */
  setSolo(solo: boolean): void {
    this.solo = solo;
    if (this.soloButton) {
      this.soloButton.classList.toggle('active', solo);
    }
  }

  /**
   * Set bypass state
   */
  setBypassed(bypassed: boolean): void {
    this.bypassed = bypassed;
    if (this.bypassButton) {
      this.bypassButton.classList.toggle('active', bypassed);
    }
  }

  /**
   * Destroy the menubar
   */
  destroy(): void {
    this.container.innerHTML = '';
  }
}
