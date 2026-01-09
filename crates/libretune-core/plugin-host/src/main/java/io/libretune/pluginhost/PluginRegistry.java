package io.libretune.pluginhost;

import com.efiAnalytics.plugin.ApplicationPlugin;

import java.io.File;
import java.net.URL;
import java.net.URLClassLoader;
import java.util.*;
import java.util.jar.JarFile;
import java.util.jar.Manifest;

/**
 * Manages loading and unloading of TunerStudio plugins.
 */
public class PluginRegistry {
    private final Map<String, LoadedPlugin> plugins = new HashMap<>();
    private final ControllerAccessImpl controllerAccess;
    
    private static void log(String message) {
        System.err.println("[PluginRegistry] " + message);
    }
    
    private static void logError(String message, Throwable e) {
        System.err.println("[PluginRegistry] ERROR: " + message);
        if (e != null) {
            e.printStackTrace(System.err);
        }
    }
    
    public PluginRegistry(ControllerAccessImpl controllerAccess) {
        this.controllerAccess = controllerAccess;
    }
    
    /**
     * Load a plugin from a JAR file.
     */
    public PluginInfo loadPlugin(String jarPath) throws Exception {
        File jarFile = new File(jarPath);
        if (!jarFile.exists()) {
            throw new IllegalArgumentException("JAR file not found: " + jarPath);
        }
        
        log("Loading plugin from: " + jarPath);
        
        // Read manifest to find plugin class
        String pluginClassName;
        try (JarFile jar = new JarFile(jarFile)) {
            Manifest manifest = jar.getManifest();
            if (manifest == null) {
                throw new IllegalArgumentException("JAR has no manifest");
            }
            
            pluginClassName = manifest.getMainAttributes().getValue("TunerStudio-Plugin");
            if (pluginClassName == null) {
                // Try alternative manifest entries
                pluginClassName = manifest.getMainAttributes().getValue("ApplicationPlugin");
            }
            if (pluginClassName == null) {
                pluginClassName = manifest.getMainAttributes().getValue("Plugin-Class");
            }
            if (pluginClassName == null) {
                throw new IllegalArgumentException("JAR manifest missing TunerStudio-Plugin or ApplicationPlugin entry");
            }
        }
        
        log("Found plugin class: " + pluginClassName);
        
        // Create classloader for the plugin
        URL[] urls = { jarFile.toURI().toURL() };
        URLClassLoader classLoader = new URLClassLoader(urls, getClass().getClassLoader());
        
        // Load and instantiate plugin class
        Class<?> pluginClass = classLoader.loadClass(pluginClassName);
        if (!ApplicationPlugin.class.isAssignableFrom(pluginClass)) {
            classLoader.close();
            throw new IllegalArgumentException("Class does not implement ApplicationPlugin: " + pluginClassName);
        }
        
        ApplicationPlugin plugin = (ApplicationPlugin) pluginClass.getDeclaredConstructor().newInstance();
        
        // Initialize plugin
        plugin.initialize(controllerAccess);
        
        // Extract plugin info
        PluginInfo info = new PluginInfo();
        info.id = plugin.getIdName();
        info.displayName = plugin.getDisplayName();
        info.description = plugin.getDescription();
        info.version = plugin.getVersion();
        info.pluginType = PluginInfo.pluginTypeToString(plugin.getPluginType());
        info.jarPath = jarPath;
        info.helpUrl = plugin.getHelpUrl();
        
        // Store loaded plugin
        LoadedPlugin loaded = new LoadedPlugin();
        loaded.info = info;
        loaded.plugin = plugin;
        loaded.classLoader = classLoader;
        
        plugins.put(info.id, loaded);
        
        log("Plugin loaded: " + info.displayName + " v" + info.version);
        
        return info;
    }
    
    /**
     * Unload a plugin by ID.
     */
    public void unloadPlugin(String pluginId) {
        LoadedPlugin loaded = plugins.remove(pluginId);
        if (loaded != null) {
            try {
                loaded.plugin.close();
                loaded.classLoader.close();
                log("Plugin unloaded: " + pluginId);
            } catch (Exception e) {
                logError("Error unloading plugin: " + e.getMessage(), e);
            }
        }
    }
    
    /**
     * Unload all plugins.
     */
    public void unloadAll() {
        for (String id : new ArrayList<>(plugins.keySet())) {
            unloadPlugin(id);
        }
    }
    
    /**
     * Get a loaded plugin by ID.
     */
    public ApplicationPlugin getPlugin(String pluginId) {
        LoadedPlugin loaded = plugins.get(pluginId);
        return loaded != null ? loaded.plugin : null;
    }
    
    /**
     * Get info for all loaded plugins.
     */
    public List<PluginInfo> getLoadedPlugins() {
        List<PluginInfo> result = new ArrayList<>();
        for (LoadedPlugin loaded : plugins.values()) {
            result.add(loaded.info);
        }
        return result;
    }
    
    /**
     * Internal wrapper for loaded plugin state.
     */
    private static class LoadedPlugin {
        PluginInfo info;
        ApplicationPlugin plugin;
        URLClassLoader classLoader;
    }
}
