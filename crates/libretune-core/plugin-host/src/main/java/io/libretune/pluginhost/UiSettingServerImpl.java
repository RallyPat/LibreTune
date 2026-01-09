package io.libretune.pluginhost;

import com.efiAnalytics.plugin.ecu.servers.UiSettingServer;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.BiConsumer;

/**
 * Implementation of UiSettingServer for plugin preferences storage.
 * Settings are stored in memory and synced to LibreTune via notifications.
 */
public class UiSettingServerImpl implements UiSettingServer {
    private static void log(String message) {
        System.err.println("[UiSettingServer] " + message);
    }
    
    private final String ecuConfigName;
    private final Map<String, Object> settings = new HashMap<>();
    private final List<String> tableNames = new ArrayList<>();
    private final List<String> curveNames = new ArrayList<>();
    private final List<String> gaugeNames = new ArrayList<>();
    private final List<String> dialogNames = new ArrayList<>();
    private final List<String> panelNames = new ArrayList<>();
    private final List<String> logFieldNames = new ArrayList<>();
    private final List<String> settingGroups = new ArrayList<>();
    private final BiConsumer<String, Object> notificationSender;
    
    public UiSettingServerImpl(String ecuConfigName, BiConsumer<String, Object> notificationSender) {
        this.ecuConfigName = ecuConfigName;
        this.notificationSender = notificationSender;
    }
    
    @Override
    public List<String> getUiTable() {
        return tableNames;
    }
    
    @Override
    public List<String> getUiCurves() {
        return curveNames;
    }
    
    @Override
    public List<String> getUiGauges() {
        return gaugeNames;
    }
    
    @Override
    public List<String> getUiDialogs() {
        return dialogNames;
    }
    
    @Override
    public List<String> getUiPanelNames() {
        return panelNames;
    }
    
    @Override
    public List<String> getUiLogFieldNames() {
        return logFieldNames;
    }
    
    @Override
    public List<String> getSettingGroups() {
        return settingGroups;
    }
    
    /**
     * Set available table names (called from LibreTune).
     */
    public void setTableNames(List<String> names) {
        tableNames.clear();
        tableNames.addAll(names);
    }
    
    /**
     * Set available curve names (called from LibreTune).
     */
    public void setCurveNames(List<String> names) {
        curveNames.clear();
        curveNames.addAll(names);
    }
    
    /**
     * Set available gauge names (called from LibreTune).
     */
    public void setGaugeNames(List<String> names) {
        gaugeNames.clear();
        gaugeNames.addAll(names);
    }
    
    /**
     * Set available dialog names (called from LibreTune).
     */
    public void setDialogNames(List<String> names) {
        dialogNames.clear();
        dialogNames.addAll(names);
    }
    
    @Override
    public String getString(String key) {
        Object value = settings.get(key);
        return value != null ? value.toString() : null;
    }
    
    @Override
    public String getString(String key, String defaultValue) {
        String value = getString(key);
        return value != null ? value : defaultValue;
    }
    
    @Override
    public void setString(String key, String value) {
        settings.put(key, value);
        notifySetting(key, value);
    }
    
    @Override
    public int getInt(String key, int defaultValue) {
        Object value = settings.get(key);
        if (value instanceof Number) {
            return ((Number) value).intValue();
        }
        return defaultValue;
    }
    
    @Override
    public void setInt(String key, int value) {
        settings.put(key, value);
        notifySetting(key, value);
    }
    
    @Override
    public boolean getBoolean(String key, boolean defaultValue) {
        Object value = settings.get(key);
        if (value instanceof Boolean) {
            return (Boolean) value;
        }
        return defaultValue;
    }
    
    @Override
    public void setBoolean(String key, boolean value) {
        settings.put(key, value);
        notifySetting(key, value);
    }
    
    @Override
    public double getDouble(String key, double defaultValue) {
        Object value = settings.get(key);
        if (value instanceof Number) {
            return ((Number) value).doubleValue();
        }
        return defaultValue;
    }
    
    @Override
    public void setDouble(String key, double value) {
        settings.put(key, value);
        notifySetting(key, value);
    }
    
    private void notifySetting(String key, Object value) {
        log("Setting " + key + " = " + value);
        notificationSender.accept("uiSettingUpdate", Map.of(
            "ecuConfigName", ecuConfigName,
            "key", key,
            "value", value
        ));
    }
    
    /**
     * Update settings from LibreTune.
     */
    public void loadSettings(Map<String, Object> newSettings) {
        settings.clear();
        settings.putAll(newSettings);
    }
}
