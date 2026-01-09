package com.efiAnalytics.plugin.ecu.servers;

import java.util.List;

/**
 * TunerStudio Plugin API - UiSettingServer interface.
 * Provides access to UI settings/preferences storage.
 * 
 * This is a LibreTune open-source implementation based on 
 * publicly documented API. Not derived from proprietary code.
 */
public interface UiSettingServer {
    
    /**
     * Get the list of UI table names available.
     * @return List of table names
     */
    List<String> getUiTable();
    
    /**
     * Get the list of UI curve names available.
     * @return List of curve names
     */
    List<String> getUiCurves();
    
    /**
     * Get the list of UI gauge names available.
     * @return List of gauge names
     */
    List<String> getUiGauges();
    
    /**
     * Get the list of UI dialog names available.
     * @return List of dialog names
     */
    List<String> getUiDialogs();
    
    /**
     * Get the list of UI panel names available.
     * @return List of panel names
     */
    List<String> getUiPanelNames();
    
    /**
     * Get the list of UI log field names available.
     * @return List of log field names
     */
    List<String> getUiLogFieldNames();
    
    /**
     * Get the list of settingGroups available.
     * @return List of setting group names
     */
    List<String> getSettingGroups();
    
    /**
     * Get a string setting.
     * @param key The setting key
     * @return The setting value, or null if not found
     */
    String getString(String key);
    
    /**
     * Get a string setting with default.
     * @param key The setting key
     * @param defaultValue The default value
     * @return The setting value, or defaultValue if not found
     */
    String getString(String key, String defaultValue);
    
    /**
     * Set a string setting.
     * @param key The setting key
     * @param value The setting value
     */
    void setString(String key, String value);
    
    /**
     * Get an integer setting.
     * @param key The setting key
     * @param defaultValue The default value
     * @return The setting value, or defaultValue if not found
     */
    int getInt(String key, int defaultValue);
    
    /**
     * Set an integer setting.
     * @param key The setting key
     * @param value The setting value
     */
    void setInt(String key, int value);
    
    /**
     * Get a boolean setting.
     * @param key The setting key
     * @param defaultValue The default value
     * @return The setting value, or defaultValue if not found
     */
    boolean getBoolean(String key, boolean defaultValue);
    
    /**
     * Set a boolean setting.
     * @param key The setting key
     * @param value The setting value
     */
    void setBoolean(String key, boolean value);
    
    /**
     * Get a double setting.
     * @param key The setting key
     * @param defaultValue The default value
     * @return The setting value, or defaultValue if not found
     */
    double getDouble(String key, double defaultValue);
    
    /**
     * Set a double setting.
     * @param key The setting key
     * @param value The setting value
     */
    void setDouble(String key, double value);
}
