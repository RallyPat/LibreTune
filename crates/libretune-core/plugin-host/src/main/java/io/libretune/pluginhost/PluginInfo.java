package io.libretune.pluginhost;

/**
 * Plugin metadata.
 */
public class PluginInfo {
    public String id;
    public String displayName;
    public String description;
    public String version;
    public String pluginType;  // "DIALOG_WIDGET", "PERSISTENT_DIALOG_PANEL", "TAB_PANEL"
    public String jarPath;
    public String helpUrl;
    
    /**
     * Convert integer plugin type to string.
     */
    public static String pluginTypeToString(int type) {
        switch (type) {
            case 0: return "DIALOG_WIDGET";
            case 1: return "PERSISTENT_DIALOG_PANEL";
            case 2: return "TAB_PANEL";
            default: return "UNKNOWN";
        }
    }
}
