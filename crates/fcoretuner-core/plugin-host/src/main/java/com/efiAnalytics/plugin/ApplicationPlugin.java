package com.efiAnalytics.plugin;

import com.efiAnalytics.plugin.ecu.ControllerAccess;
import javax.swing.JComponent;

/**
 * TunerStudio Plugin API - ApplicationPlugin interface.
 * This is a compatibility stub for loading TunerStudio plugins.
 */
public interface ApplicationPlugin {
    
    // Plugin types (int constants as used by actual plugins)
    int DIALOG_WIDGET = 0;
    int PERSISTENT_DIALOG_PANEL = 1;
    int TAB_PANEL = 2;
    
    /**
     * Get the unique ID name for this plugin.
     */
    String getIdName();
    
    /**
     * Get the plugin type.
     * @return One of DIALOG_WIDGET (0), PERSISTENT_DIALOG_PANEL (1), or TAB_PANEL (2)
     */
    int getPluginType();
    
    /**
     * Get the display name for this plugin.
     */
    String getDisplayName();
    
    /**
     * Get a description of this plugin.
     */
    String getDescription();
    
    /**
     * Initialize the plugin with controller access.
     * @param controllerAccess The gateway to ECU data
     */
    void initialize(ControllerAccess controllerAccess);
    
    /**
     * Display the plugin for a specific controller.
     * @param controllerSignature The ECU signature
     */
    default void displayPlugin(String controllerSignature) {}
    
    /**
     * Check if the menu item for this plugin should be enabled.
     */
    default boolean isMenuEnabled() { return true; }
    
    /**
     * Get the plugin's UI panel.
     * @return The Swing component for this plugin's UI
     */
    JComponent getPluginPanel();
    
    /**
     * Close the plugin and release resources.
     */
    void close();
    
    /**
     * Get the help URL for this plugin.
     */
    default String getHelpUrl() { return null; }
    
    /**
     * Get the version string for this plugin.
     */
    String getVersion();
    
    /**
     * Get the required plugin spec version.
     */
    default String getRequiredPluginSpec() { return "1.0"; }
}
