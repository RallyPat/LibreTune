/**
 * Shared styles module
 * 
 * Import this in any top-level component (App, PopOutWindow, etc.)
 * to ensure all CSS custom properties and global styles are available.
 */

// Self-hosted fonts — avoids external requests from tauri:// custom scheme
import '@fontsource/inter/400.css';
import '@fontsource/inter/500.css';
import '@fontsource/inter/600.css';
import '@fontsource/inter/700.css';
import '@fontsource/jetbrains-mono/400.css';
import '@fontsource/jetbrains-mono/500.css';
import '@fontsource/jetbrains-mono/600.css';

// Theme CSS custom properties (--bg-primary, --text-primary, etc.)
import '../themes/base.css';
import '../themes/industrial.css';
import '../themes/shell.css';

// Global app styles
import '../App.css';
