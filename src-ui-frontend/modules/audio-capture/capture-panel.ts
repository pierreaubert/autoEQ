// Capture Panel Web Component
// A self-contained web component for the audio capture interface

export class CapturePanel extends HTMLElement {
  constructor() {
    super();
    this.render();
  }

  connectedCallback() {
    // Dispatch event to notify that the panel has been rendered
    setTimeout(() => {
      const event = new CustomEvent('capturePanelRendered', {
        bubbles: true,
        composed: true,
      });
      this.dispatchEvent(event);
    }, 0);
  }

  private render(): void {
    this.innerHTML = `
      <div id="capture_panel" class="capture-panel">
        <div class="capture-panel-body">
          <!-- Capture Controls -->
          <div class="capture-controls-block">
            <div class="capture-controls-row">
              <div class="capture-control-group">
                <div class="label-with-badge">
                  <label for="modal_capture_device">Input:</label>
                  <div class="badge-group">
                    <span id="input_channels_info" class="channel-count-badge">? ch</span>
                    <span id="modal_capture_sample_rate" class="info-badge sample-rate-badge">48kHz</span>
                    <span id="modal_capture_bit_depth" class="info-badge bit-depth-badge">24</span>
                    <span id="modal_capture_spl" class="info-badge spl-badge" style="display: none;">-- dB</span>
                    <button id="input_routing_btn" class="routing-button" title="Configure input channel routing">
                      <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
                        <line x1="0" y1="0" x2="0" y2="16" stroke="currentColor" stroke-width="1"/>
                        <line x1="5.333" y1="0" x2="5.333" y2="16" stroke="currentColor" stroke-width="1"/>
                        <line x1="10.666" y1="0" x2="10.666" y2="16" stroke="currentColor" stroke-width="1"/>
                        <line x1="16" y1="0" x2="16" y2="16" stroke="currentColor" stroke-width="1"/>
                        <line x1="0" y1="0" x2="16" y2="0" stroke="currentColor" stroke-width="1"/>
                        <line x1="0" y1="5.333" x2="16" y2="5.333" stroke="currentColor" stroke-width="1"/>
                        <line x1="0" y1="10.666" x2="16" y2="10.666" stroke="currentColor" stroke-width="1"/>
                        <line x1="0" y1="16" x2="16" y2="16" stroke="currentColor" stroke-width="1"/>
                        <circle cx="2.666" cy="2.666" r="1.5" fill="#57F287"/>
                        <circle cx="8" cy="8" r="1.5" fill="#57F287"/>
                        <circle cx="13.333" cy="13.333" r="1.5" fill="#57F287"/>
                      </svg>
                    </button>
                  </div>
                </div>
                <select id="modal_capture_device" class="capture-device-select">
                  <option value="">Loading devices...</option>
                </select>
              </div>

              <div class="capture-control-group capture-volume-group">
                <label for="modal_capture_volume">Input Gain:</label>
                <div class="volume-slider-container">
                  <input type="range" id="modal_capture_volume" class="volume-slider" min="0" max="100" value="70" step="1">
                  <div class="volume-value" id="modal_capture_volume_value">70%</div>
                </div>
              </div>

              <div class="capture-control-group">
                <label for="capture_calibration_file">Calibration:</label>
                <div class="capture-calibration-inline">
                  <input type="file" id="capture_calibration_file" accept=".csv,.txt" style="display: none">
                  <button type="button" id="capture_calibration_btn" class="btn btn-outline btn-sm">
                    📁 Load File
                  </button>
                  <button type="button" id="capture_calibration_clear" class="btn btn-outline btn-sm" style="display: none">
                    ✕ Clear
                  </button>
                </div>
              </div>

              <div class="capture-control-group">
                <div class="label-with-badge">
                  <label for="modal_output_device">Output:</label>
                  <div class="badge-group">
                    <span id="output_channels_info" class="channel-count-badge">? ch</span>
                    <span id="modal_output_sample_rate" class="info-badge sample-rate-badge">48kHz</span>
                    <span id="modal_output_bit_depth" class="info-badge bit-depth-badge">24</span>
                    <button id="output_routing_btn" class="routing-button" title="Configure output channel routing">
                      <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
                        <line x1="0" y1="0" x2="0" y2="16" stroke="currentColor" stroke-width="1"/>
                        <line x1="5.333" y1="0" x2="5.333" y2="16" stroke="currentColor" stroke-width="1"/>
                        <line x1="10.666" y1="0" x2="10.666" y2="16" stroke="currentColor" stroke-width="1"/>
                        <line x1="16" y1="0" x2="16" y2="16" stroke="currentColor" stroke-width="1"/>
                        <line x1="0" y1="0" x2="16" y2="0" stroke="currentColor" stroke-width="1"/>
                        <line x1="0" y1="5.333" x2="16" y2="5.333" stroke="currentColor" stroke-width="1"/>
                        <line x1="0" y1="10.666" x2="16" y2="10.666" stroke="currentColor" stroke-width="1"/>
                        <line x1="0" y1="16" x2="16" y2="16" stroke="currentColor" stroke-width="1"/>
                        <circle cx="2.666" cy="2.666" r="1.5" fill="#57F287"/>
                        <circle cx="8" cy="8" r="1.5" fill="#57F287"/>
                        <circle cx="13.333" cy="13.333" r="1.5" fill="#57F287"/>
                      </svg>
                    </button>
                  </div>
                </div>
                <select id="modal_output_device" class="output-device-select">
                  <option value="default" selected>System Default</option>
                </select>
              </div>

              <div class="capture-control-group capture-volume-group">
                <label for="modal_output_volume">Output Gain:</label>
                <div class="volume-slider-container">
                  <input type="range" id="modal_output_volume" class="volume-slider" min="0" max="100" value="50" step="1">
                  <div class="volume-value" id="modal_output_volume_value">50%</div>
                </div>
              </div>
            </div>

            <!-- Second row: Signal parameters (slim inline layout) -->
            <div class="capture-controls-row capture-controls-row-slim">
              <div class="capture-control-inline">
                <label for="modal_output_channel">Channel:</label>
                <select id="modal_output_channel" class="output-channel-select">
                  <option value="all" selected>All Channels</option>
                </select>
              </div>

              <div class="capture-control-inline">
                <label for="modal_signal_type">Signal:</label>
                <select id="modal_signal_type" class="signal-type-select">
                  <option value="sweep" selected>Frequency Sweep</option>
                  <option value="white">White Noise</option>
                  <option value="pink">Pink Noise</option>
                </select>
              </div>

              <div class="capture-control-inline" id="modal_sweep_duration_container">
                <label for="modal_sweep_duration">Duration:</label>
                <select id="modal_sweep_duration" class="sweep-duration-select">
                  <option value="5">5 seconds</option>
                  <option value="10" selected>10 seconds</option>
                  <option value="15">15 seconds</option>
                  <option value="20">20 seconds</option>
                </select>
              </div>
            </div>
          </div>

          <!-- Graph Display with Records Sidebar -->
          <div class="capture-main-area">
            <!-- Records Management Sidebar -->
            <div id="capture_records_sidebar" class="capture-records-sidebar">
              <div class="records-header">
                <h4>📋 Saved Records</h4>
                <button id="records_toggle" class="records-toggle-btn" title="Toggle records panel">
                  ◀
                </button>
              </div>
              <div class="records-actions">
                <button id="records_select_all" class="btn btn-sm btn-outline">Select All</button>
                <button id="records_deselect_all" class="btn btn-sm btn-outline">Deselect All</button>
                <button id="records_delete_selected" class="btn btn-sm btn-danger">🗑️ Delete</button>
              </div>
              <div id="capture_records_list" class="capture-records-list">
                <!-- Records will be dynamically populated here -->
              </div>
            </div>

            <!-- Graph Container -->
            <div class="capture-graph-container">
              <canvas id="capture_modal_graph" class="capture-modal-graph"></canvas>
              <div id="capture_modal_placeholder" class="capture-graph-placeholder">
                <div class="capture-placeholder-content">
                  <div class="capture-placeholder-icon">📊</div>
                  <h4>Frequency & Phase Response Graph</h4>
                  <p>Click "Start Capture" to begin audio measurement with phase analysis</p>
                </div>
              </div>
              <!-- Progress bar -->
              <div id="capture_modal_progress" class="capture-progress" style="display: none;">
                <div id="capture_modal_progress_fill" class="capture-progress-fill"></div>
              </div>
              <!-- Status message -->
              <div id="capture_modal_status" class="capture-status"></div>
            </div>
          </div>
        </div>

        <!-- Bottom Controls Bar -->
        <div class="capture-bottom-controls">
            <div class="capture-bottom-left">
              <!-- Phase and Smoothing Controls -->
              <label class="capture-phase-toggle">
                <input type="checkbox" id="capture_phase_toggle" checked>
                <span>Show Phase</span>
              </label>
              <label class="capture-smoothing-control">
                <span>Smoothing:</span>
                <select id="capture_smoothing_select" class="capture-smoothing-select">
                  <option value="1">1/1 octave</option>
                  <option value="2">1/2 octave</option>
                  <option value="3" selected>1/3 octave</option>
                  <option value="4">1/4 octave</option>
                  <option value="6">1/6 octave</option>
                  <option value="8">1/8 octave</option>
                  <option value="12">1/12 octave</option>
                  <option value="24">1/24 octave</option>
                </select>
              </label>
              <!-- Channel Display Controls -->
              <label class="capture-channel-control">
                <span>Show:</span>
                <select id="capture_channel_select" class="capture-channel-select">
                  <option value="combined" selected>Combined</option>
                  <option value="left">Left Channel</option>
                  <option value="right">Right Channel</option>
                  <option value="average">L/R Average</option>
                  <option value="all">All Channels</option>
                </select>
              </label>
            </div>
            <div class="capture-bottom-right">
              <!-- Action Buttons -->
              <button id="capture_modal_start" class="btn btn-primary capture-start-btn">
                🎤 Start Capture
              </button>
              <button id="capture_modal_stop" class="btn btn-danger capture-stop-btn" style="display: none">
                ⏹️ Stop Capture
              </button>
              <button id="capture_modal_export" class="btn btn-secondary capture-export-btn" style="display: none">
                💾 Export CSV
              </button>
            </div>
          </div>
      </div>
    `;
  }
}

// Register the custom element
if (!customElements.get('capture-panel')) {
  customElements.define('capture-panel', CapturePanel);
}
