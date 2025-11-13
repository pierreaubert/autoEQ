// Base Plugin Implementation
// Provides common functionality for all plugins

import type {
  IPlugin,
  PluginMetadata,
  PluginState,
  PluginConfig,
} from './plugin-types';

import { PluginEvent } from './plugin-types';

/**
 * Base plugin class
 * Implements common plugin functionality
 */
export abstract class BasePlugin implements IPlugin {
  // Metadata
  public abstract readonly metadata: PluginMetadata;

  // Container
  protected container: HTMLElement | null = null;
  protected config: PluginConfig = {};

  // State
  protected state: PluginState = {
    enabled: true,
    bypassed: false,
    parameters: {},
  };

  // Event listeners
  private eventListeners: Map<string, Set<(...args: any[]) => void>> = new Map();

  /**
   * Initialize the plugin
   */
  initialize(container: HTMLElement, config: PluginConfig = {}): void {
    this.container = container;
    this.config = config;

    // Apply initial state if provided
    if (config.initialState) {
      this.setState(config.initialState);
    }

    // Render the plugin
    this.render(config.standalone ?? true);
  }

  /**
   * Destroy the plugin
   */
  destroy(): void {
    if (this.container) {
      this.container.innerHTML = '';
      this.container = null;
    }
    this.eventListeners.clear();
  }

  /**
   * Get current state
   */
  getState(): PluginState {
    return { ...this.state };
  }

  /**
   * Set state (partial update)
   */
  setState(newState: Partial<PluginState>): void {
    const oldState = { ...this.state };
    this.state = { ...this.state, ...newState };

    // Notify config callback
    if (this.config.onStateChange) {
      this.config.onStateChange(this.state);
    }

    // Emit event
    this.emit(PluginEvent.StateChanged as string, this.state, oldState);

    // Handle specific state changes
    if (newState.bypassed !== undefined && newState.bypassed !== oldState.bypassed) {
      this.handleBypassChange(newState.bypassed);
    }

    if (newState.parameters !== undefined) {
      this.handleParameterChange(newState.parameters, oldState.parameters);
    }
  }

  /**
   * Render the plugin UI
   */
  abstract render(standalone: boolean): void;

  /**
   * Handle bypass state change
   */
  protected handleBypassChange(bypassed: boolean): void {
    if (this.config.onBypass) {
      this.config.onBypass(bypassed);
    }
    this.emit(PluginEvent.Bypassed as string, bypassed);
  }

  /**
   * Handle parameter change
   */
  protected handleParameterChange(newParams: Record<string, any>, oldParams: Record<string, any>): void {
    // Find changed parameters
    const changed: Record<string, { old: any; new: any }> = {};
    for (const key in newParams) {
      if (newParams[key] !== oldParams[key]) {
        changed[key] = { old: oldParams[key], new: newParams[key] };
      }
    }

    if (Object.keys(changed).length > 0) {
      this.emit(PluginEvent.ParameterChanged as string, changed);
    }
  }

  /**
   * Register event listener
   */
  on(event: string, callback: (...args: any[]) => void): void {
    if (!this.eventListeners.has(event)) {
      this.eventListeners.set(event, new Set());
    }
    this.eventListeners.get(event)!.add(callback);
  }

  /**
   * Unregister event listener
   */
  off(event: string, callback: (...args: any[]) => void): void {
    const listeners = this.eventListeners.get(event);
    if (listeners) {
      listeners.delete(callback);
    }
  }

  /**
   * Emit event
   */
  emit(event: string, ...args: any[]): void {
    const listeners = this.eventListeners.get(event);
    if (listeners) {
      listeners.forEach((callback) => {
        try {
          callback(...args);
        } catch (error) {
          console.error(`[Plugin ${this.metadata.name}] Error in event listener for ${event}:`, error);
        }
      });
    }
  }

  /**
   * Update a single parameter
   */
  protected updateParameter(key: string, value: any): void {
    const newParameters = { ...this.state.parameters, [key]: value };
    this.setState({ parameters: newParameters });
  }

  /**
   * Get a single parameter value
   */
  protected getParameter<T>(key: string, defaultValue?: T): T | undefined {
    return (this.state.parameters[key] as T) ?? defaultValue;
  }

  /**
   * Check if plugin is bypassed
   */
  isBypassed(): boolean {
    return this.state.bypassed;
  }

  /**
   * Check if plugin is enabled
   */
  isEnabled(): boolean {
    return this.state.enabled;
  }

  /**
   * Set bypass state
   */
  setBypass(bypassed: boolean): void {
    this.setState({ bypassed });
  }

  /**
   * Toggle bypass state
   */
  toggleBypass(): void {
    this.setBypass(!this.state.bypassed);
  }
}
