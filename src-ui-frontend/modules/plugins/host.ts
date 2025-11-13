// Plugin Host Component
// Container for managing multiple plugins with menubar, hosting bar, and display box

import './host.css';
import { PluginMenubar, type PluginMenubarCallbacks } from './plugin-menubar';
import { LevelMeter } from './level-meter';
import type { IPlugin, MenubarConfig, LevelMeterData, LUFSMeterData } from './plugin-types';

export interface HostConfig {
  name: string;
  allowedPlugins?: string[];     // List of allowed plugin types
  maxPlugins?: number;            // Maximum number of plugins (default: unlimited)
  showLevelMeters?: boolean;      // Show level meters in right panel
  showLUFS?: boolean;             // Show LUFS meter
  showVolumeControl?: boolean;    // Show volume control
  menubarConfig?: MenubarConfig;
}

export interface HostCallbacks {
  onPluginAdd?: (plugin: IPlugin) => void;
  onPluginRemove?: (plugin: IPlugin) => void;
  onPluginSelect?: (plugin: IPlugin | null) => void;
  onVolumeChange?: (volume: number) => void;
}

/**
 * Plugin Host
 * Manages a collection of plugins with unified UI
 */
export class PluginHost {
  private container: HTMLElement;
  private config: HostConfig;
  private callbacks: HostCallbacks;

  // UI components
  private menubar: PluginMenubar | null = null;
  private levelMeter: LevelMeter | null = null;

  // UI elements
  private hostingBar: HTMLElement | null = null;
  private displayBox: HTMLElement | null = null;
  private displayLeft: HTMLElement | null = null;
  private displayRight: HTMLElement | null = null;
  private pluginSlotsContainer: HTMLElement | null = null;

  // State
  private plugins: IPlugin[] = [];
  private selectedPlugin: IPlugin | null = null;
  private volume: number = 1.0;
  private muted: boolean = false;
  private monitoringMode: 'input' | 'output' = 'output';

  constructor(container: HTMLElement, config: HostConfig, callbacks: HostCallbacks = {}) {
    this.container = container;
    this.config = {
      allowedPlugins: config.allowedPlugins ?? [],
      maxPlugins: config.maxPlugins ?? Infinity,
      showLevelMeters: config.showLevelMeters ?? true,
      showLUFS: config.showLUFS ?? true,
      showVolumeControl: config.showVolumeControl ?? true,
      menubarConfig: config.menubarConfig ?? {},
      ...config,
    };
    this.callbacks = callbacks;

    this.render();
  }

  /**
   * Render the host UI
   */
  private render(): void {
    this.container.classList.add('plugin-host');
    this.container.innerHTML = `
      <div class="plugin-host-container">
        <!-- Menubar -->
        <div class="plugin-host-menubar"></div>

        <!-- Hosting Bar -->
        <div class="plugin-hosting-bar">
          <div class="plugin-slots"></div>
          <button class="plugin-add-button" title="Add Plugin">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
              <path d="M8 2v12M2 8h12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
            </svg>
          </button>
        </div>

        <!-- Display Box -->
        <div class="plugin-display-box">
          <!-- Left: Active Plugin Display -->
          <div class="plugin-display-left">
            <div class="plugin-display-placeholder">
              <svg width="48" height="48" viewBox="0 0 48 48" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="8" y="8" width="32" height="32" rx="4"/>
                <circle cx="24" cy="20" r="4"/>
                <path d="M16 32h16"/>
              </svg>
              <p>No plugin selected</p>
            </div>
          </div>

          <!-- Right: Meters & Controls -->
          <div class="plugin-display-right">
            ${this.renderRightPanel()}
          </div>
        </div>
      </div>
    `;

    this.cacheElements();
    this.initializeComponents();
    this.attachEventListeners();
  }

  /**
   * Render right panel (LUFS, meters, volume)
   */
  private renderRightPanel(): string {
    return `
      ${this.config.showLUFS ? this.renderLUFSMeter() : ''}
      ${this.config.showLevelMeters ? this.renderMonitoringToggle() : ''}
      ${this.config.showLevelMeters ? this.renderLevelMeters() : ''}
      ${this.config.showVolumeControl ? this.renderVolumeControl() : ''}
    `;
  }

  /**
   * Render monitoring toggle (input/output)
   */
  private renderMonitoringToggle(): string {
    return `
      <div class="monitoring-toggle">
        <button class="monitoring-button ${this.monitoringMode === 'input' ? 'active' : ''}" data-mode="input">
          Input
        </button>
        <button class="monitoring-button ${this.monitoringMode === 'output' ? 'active' : ''}" data-mode="output">
          Output
        </button>
      </div>
    `;
  }

  /**
   * Render LUFS meter
   */
  private renderLUFSMeter(): string {
    return `
      <div class="lufs-meter">
        <div class="lufs-label">LUFS</div>
        <div class="lufs-value-container">
          <div class="lufs-row">
            <span class="lufs-type">M</span>
            <span class="lufs-value" data-lufs="momentary">-∞</span>
          </div>
          <div class="lufs-row">
            <span class="lufs-type">S</span>
            <span class="lufs-value" data-lufs="shortTerm">-∞</span>
          </div>
          <div class="lufs-row">
            <span class="lufs-type">I</span>
            <span class="lufs-value" data-lufs="integrated">-∞</span>
          </div>
        </div>
      </div>
    `;
  }

  /**
   * Render level meters
   */
  private renderLevelMeters(): string {
    return `
      <div class="level-meters-container">
        <canvas class="level-meters-canvas" width="80" height="200"></canvas>
      </div>
    `;
  }

  /**
   * Render volume control
   */
  private renderVolumeControl(): string {
    return `
      <div class="volume-control">
        <div class="volume-label">Volume</div>
        <input type="range" class="volume-slider" min="0" max="100" value="${this.volume * 100}" />
        <div class="volume-value">${Math.round(this.volume * 100)}%</div>
      </div>
    `;
  }

  /**
   * Cache DOM elements
   */
  private cacheElements(): void {
    this.hostingBar = this.container.querySelector('.plugin-hosting-bar');
    this.displayBox = this.container.querySelector('.plugin-display-box');
    this.displayLeft = this.container.querySelector('.plugin-display-left');
    this.displayRight = this.container.querySelector('.plugin-display-right');
    this.pluginSlotsContainer = this.container.querySelector('.plugin-slots');
  }

  /**
   * Initialize components
   */
  private initializeComponents(): void {
    // Initialize menubar
    const menubarContainer = this.container.querySelector('.plugin-host-menubar') as HTMLElement;
    if (menubarContainer) {
      const menubarCallbacks: PluginMenubarCallbacks = {
        onMatrix: () => this.showMatrixDialog(),
        onMute: (muted) => this.setMuted(muted),
      };
      this.menubar = new PluginMenubar(menubarContainer, this.config.name, this.config.menubarConfig, menubarCallbacks);
    }

    // Initialize level meters
    if (this.config.showLevelMeters) {
      const canvas = this.container.querySelector('.level-meters-canvas') as HTMLCanvasElement;
      if (canvas) {
        this.levelMeter = new LevelMeter({
          canvas,
          channels: 6, // L, R, C, LFE, SL, SR
          channelLabels: ['L', 'R', 'C', 'LFE', 'SL', 'SR'],
        });
      }
    }
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Add plugin button
    const addButton = this.container.querySelector('.plugin-add-button') as HTMLButtonElement;
    if (addButton) {
      addButton.addEventListener('click', () => this.showPluginSelector());
    }

    // Volume slider
    const volumeSlider = this.container.querySelector('.volume-slider') as HTMLInputElement;
    if (volumeSlider) {
      volumeSlider.addEventListener('input', (e) => {
        const value = parseInt((e.target as HTMLInputElement).value, 10) / 100;
        this.setVolume(value);
      });
    }

    // Monitoring toggle buttons
    const monitoringButtons = this.container.querySelectorAll('.monitoring-button');
    monitoringButtons.forEach((button) => {
      button.addEventListener('click', (e) => {
        const mode = (e.target as HTMLElement).dataset.mode as 'input' | 'output';
        this.setMonitoringMode(mode);
      });
    });
  }

  /**
   * Add a plugin
   */
  addPlugin(plugin: IPlugin): void {
    // Check if we can add more plugins
    if (this.plugins.length >= this.config.maxPlugins!) {
      console.warn('[PluginHost] Maximum number of plugins reached');
      return;
    }

    // Check if plugin type is allowed
    if (
      this.config.allowedPlugins!.length > 0 &&
      !this.config.allowedPlugins!.includes(plugin.metadata.category)
    ) {
      console.warn('[PluginHost] Plugin type not allowed:', plugin.metadata.category);
      return;
    }

    this.plugins.push(plugin);
    this.renderPluginSlot(plugin);

    // Update level meters if plugin changes channel count
    this.updateLevelMeterChannels();

    // Callback
    if (this.callbacks.onPluginAdd) {
      this.callbacks.onPluginAdd(plugin);
    }

    // Auto-select if it's the first plugin
    if (this.plugins.length === 1) {
      this.selectPlugin(plugin);
    }
  }

  /**
   * Remove a plugin
   */
  removePlugin(plugin: IPlugin): void {
    const index = this.plugins.indexOf(plugin);
    if (index === -1) return;

    this.plugins.splice(index, 1);

    // If this was the selected plugin, clear selection
    if (this.selectedPlugin === plugin) {
      this.selectPlugin(null);
    }

    // Re-render slots
    this.renderAllPluginSlots();

    // Update level meters if plugin changes channel count
    this.updateLevelMeterChannels();

    // Cleanup plugin
    plugin.destroy();

    // Callback
    if (this.callbacks.onPluginRemove) {
      this.callbacks.onPluginRemove(plugin);
    }
  }

  /**
   * Select a plugin
   */
  selectPlugin(plugin: IPlugin | null): void {
    this.selectedPlugin = plugin;

    // Update slot highlighting
    this.updateSlotSelection();

    // Render plugin in display area
    if (this.displayLeft) {
      if (plugin) {
        this.displayLeft.innerHTML = '<div class="active-plugin-container"></div>';
        const pluginContainer = this.displayLeft.querySelector('.active-plugin-container') as HTMLElement;
        plugin.initialize(pluginContainer, { standalone: false, showMenubar: false });
      } else {
        this.displayLeft.innerHTML = `
          <div class="plugin-display-placeholder">
            <svg width="48" height="48" viewBox="0 0 48 48" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="8" y="8" width="32" height="32" rx="4"/>
              <circle cx="24" cy="20" r="4"/>
              <path d="M16 32h16"/>
            </svg>
            <p>No plugin selected</p>
          </div>
        `;
      }
    }

    // Callback
    if (this.callbacks.onPluginSelect) {
      this.callbacks.onPluginSelect(plugin);
    }
  }

  /**
   * Render a single plugin slot
   */
  private renderPluginSlot(plugin: IPlugin): void {
    if (!this.pluginSlotsContainer) return;

    const slot = document.createElement('div');
    slot.className = 'plugin-slot';
    slot.dataset.pluginId = plugin.metadata.id;

    slot.innerHTML = `
      <button class="plugin-slot-remove" title="Remove">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor">
          <path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
        </svg>
      </button>
      <div class="plugin-slot-name">${plugin.metadata.name}</div>
      <div class="plugin-slot-box"></div>
    `;

    // Click to select
    slot.addEventListener('click', (e) => {
      if (!(e.target as HTMLElement).closest('.plugin-slot-remove')) {
        this.selectPlugin(plugin);
      }
    });

    // Remove button
    const removeBtn = slot.querySelector('.plugin-slot-remove') as HTMLButtonElement;
    removeBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      this.removePlugin(plugin);
    });

    this.pluginSlotsContainer.appendChild(slot);
  }

  /**
   * Render all plugin slots
   */
  private renderAllPluginSlots(): void {
    if (!this.pluginSlotsContainer) return;

    this.pluginSlotsContainer.innerHTML = '';
    this.plugins.forEach((plugin) => this.renderPluginSlot(plugin));
    this.updateSlotSelection();
  }

  /**
   * Update slot selection highlighting
   */
  private updateSlotSelection(): void {
    const slots = this.container.querySelectorAll('.plugin-slot');
    slots.forEach((slot) => {
      const pluginId = (slot as HTMLElement).dataset.pluginId;
      const isSelected = !!(this.selectedPlugin && this.selectedPlugin.metadata.id === pluginId);
      slot.classList.toggle('selected', isSelected);
    });
  }

  /**
   * Show plugin selector dialog
   */
  private showPluginSelector(): void {
    // Available plugins
    const availablePlugins = [
      { id: 'eq', name: 'EQ', category: 'eq', description: 'Parametric Equalizer', icon: '🎚️' },
      { id: 'compressor', name: 'Compressor', category: 'dynamics', description: 'Dynamic Range Compressor', icon: '🔊' },
      { id: 'limiter', name: 'Limiter', category: 'dynamics', description: 'Peak Limiter', icon: '🛡️' },
      { id: 'upmixer', name: 'Upmixer', category: 'spatial', description: 'Stereo to 5.1 Upmixer', icon: '🔉' },
      { id: 'spectrum', name: 'Spectrum', category: 'analyzer', description: 'Frequency Spectrum Analyzer', icon: '📊' },
    ];

    // Filter by allowed plugins if specified
    const plugins = this.config.allowedPlugins!.length > 0
      ? availablePlugins.filter(p => this.config.allowedPlugins!.includes(p.category))
      : availablePlugins;

    // Create dialog
    const dialog = document.createElement('div');
    dialog.className = 'plugin-selector-overlay';
    dialog.innerHTML = `
      <div class="plugin-selector-dialog">
        <div class="plugin-selector-header">
          <h3>Add Plugin</h3>
          <button class="plugin-selector-close">×</button>
        </div>
        <div class="plugin-selector-body">
          ${plugins.map(plugin => `
            <button class="plugin-selector-item" data-plugin-id="${plugin.id}">
              <span class="plugin-icon">${plugin.icon}</span>
              <div class="plugin-info">
                <div class="plugin-name">${plugin.name}</div>
                <div class="plugin-description">${plugin.description}</div>
              </div>
            </button>
          `).join('')}
        </div>
      </div>
    `;

    document.body.appendChild(dialog);

    // Close handler
    const closeDialog = () => {
      dialog.remove();
    };

    // Close button
    const closeBtn = dialog.querySelector('.plugin-selector-close') as HTMLButtonElement;
    closeBtn.addEventListener('click', closeDialog);

    // Overlay click
    dialog.addEventListener('click', (e) => {
      if (e.target === dialog) {
        closeDialog();
      }
    });

    // Plugin selection
    const pluginItems = dialog.querySelectorAll('.plugin-selector-item');
    pluginItems.forEach(item => {
      item.addEventListener('click', () => {
        const pluginId = (item as HTMLElement).dataset.pluginId!;
        this.createPluginById(pluginId);
        closeDialog();
      });
    });
  }

  /**
   * Create plugin by ID
   */
  private createPluginById(pluginId: string): void {
    // Dynamically import and create plugin
    import('./plugin-eq').then(({ EQPlugin }) => {
      if (pluginId === 'eq') {
        const plugin = new EQPlugin();
        this.addPlugin(plugin);
      }
    });

    import('./plugin-compressor').then(({ CompressorPlugin }) => {
      if (pluginId === 'compressor') {
        const plugin = new CompressorPlugin();
        this.addPlugin(plugin);
      }
    });

    import('./plugin-limiter').then(({ LimiterPlugin }) => {
      if (pluginId === 'limiter') {
        const plugin = new LimiterPlugin();
        this.addPlugin(plugin);
      }
    });

    import('./plugin-upmixer').then(({ UpmixerPlugin }) => {
      if (pluginId === 'upmixer') {
        const plugin = new UpmixerPlugin();
        this.addPlugin(plugin);
      }
    });

    import('./plugin-spectrum').then(({ SpectrumPlugin }) => {
      if (pluginId === 'spectrum') {
        const plugin = new SpectrumPlugin();
        this.addPlugin(plugin);
      }
    });
  }

  /**
   * Show matrix routing dialog
   */
  private showMatrixDialog(): void {
    // TODO: Implement matrix routing dialog
    console.log('[PluginHost] Show matrix dialog');
  }

  /**
   * Update level meters
   */
  updateLevelMeters(data: LevelMeterData): void {
    if (this.levelMeter) {
      this.levelMeter.update(data);
    }
  }

  /**
   * Update LUFS values
   */
  updateLUFS(data: LUFSMeterData): void {
    const momentaryEl = this.container.querySelector('[data-lufs="momentary"]') as HTMLElement;
    const shortTermEl = this.container.querySelector('[data-lufs="shortTerm"]') as HTMLElement;
    const integratedEl = this.container.querySelector('[data-lufs="integrated"]') as HTMLElement;

    if (momentaryEl) momentaryEl.textContent = data.momentary.toFixed(1);
    if (shortTermEl) shortTermEl.textContent = data.shortTerm.toFixed(1);
    if (integratedEl) integratedEl.textContent = data.integrated.toFixed(1);
  }

  /**
   * Set volume
   */
  setVolume(volume: number): void {
    this.volume = Math.max(0, Math.min(1, volume));

    // Update UI
    const volumeSlider = this.container.querySelector('.volume-slider') as HTMLInputElement;
    const volumeValue = this.container.querySelector('.volume-value') as HTMLElement;
    if (volumeSlider) volumeSlider.value = String(this.volume * 100);
    if (volumeValue) volumeValue.textContent = `${Math.round(this.volume * 100)}%`;

    // Callback
    if (this.callbacks.onVolumeChange) {
      this.callbacks.onVolumeChange(this.volume);
    }
  }

  /**
   * Get volume
   */
  getVolume(): number {
    return this.volume;
  }

  /**
   * Set muted state
   */
  setMuted(muted: boolean): void {
    this.muted = muted;
    if (this.menubar) {
      this.menubar.setMuted(muted);
    }
  }

  /**
   * Get all plugins
   */
  getPlugins(): IPlugin[] {
    return [...this.plugins];
  }

  /**
   * Get selected plugin
   */
  getSelectedPlugin(): IPlugin | null {
    return this.selectedPlugin;
  }

  /**
   * Set monitoring mode (input/output)
   */
  setMonitoringMode(mode: 'input' | 'output'): void {
    this.monitoringMode = mode;

    // Update button states
    const buttons = this.container.querySelectorAll('.monitoring-button');
    buttons.forEach((button) => {
      const btnMode = (button as HTMLElement).dataset.mode;
      button.classList.toggle('active', btnMode === mode);
    });

    // Reconfigure level meters with appropriate channel count
    this.updateLevelMeterChannels();

    console.log('[PluginHost] Monitoring mode:', mode);
  }

  /**
   * Update level meter channel configuration based on monitoring mode
   */
  private updateLevelMeterChannels(): void {
    if (!this.levelMeter) return;

    const { inputChannels, outputChannels } = this.getChannelCounts();
    const channels = this.monitoringMode === 'input' ? inputChannels : outputChannels;
    const labels = this.generateChannelLabels(channels);

    this.levelMeter.reconfigure(channels, labels);
  }

  /**
   * Get input and output channel counts based on plugin chain
   */
  private getChannelCounts(): { inputChannels: number; outputChannels: number } {
    // Default: start with 2 channels (stereo input)
    let inputChannels = 2;
    let outputChannels = 2;

    // If we have plugins, check if any change channel count
    // For now, check if there's an upmixer plugin
    const hasUpmixer = this.plugins.some(p => p.metadata.category === 'spatial');

    if (hasUpmixer) {
      // Upmixer: 2ch input -> 5ch or 6ch output
      inputChannels = 2;
      outputChannels = 6; // L, R, C, LFE, SL, SR
    } else {
      // No channel-changing plugins: same in/out
      inputChannels = 2;
      outputChannels = 2;
    }

    return { inputChannels, outputChannels };
  }

  /**
   * Generate channel labels based on count
   */
  private generateChannelLabels(count: number): string[] {
    if (count === 2) {
      return ['L', 'R'];
    } else if (count === 6) {
      return ['L', 'R', 'C', 'LFE', 'SL', 'SR'];
    } else if (count === 8) {
      return ['L', 'R', 'C', 'LFE', 'SL', 'SR', 'SBL', 'SBR'];
    } else {
      return Array.from({ length: count }, (_, i) => `${i + 1}`);
    }
  }

  /**
   * Destroy the host
   */
  destroy(): void {
    // Destroy all plugins
    this.plugins.forEach((plugin) => plugin.destroy());
    this.plugins = [];

    // Destroy components
    if (this.menubar) {
      this.menubar.destroy();
      this.menubar = null;
    }

    if (this.levelMeter) {
      this.levelMeter.destroy();
      this.levelMeter = null;
    }

    // Clear container
    this.container.innerHTML = '';
    this.container.classList.remove('plugin-host');
  }
}
