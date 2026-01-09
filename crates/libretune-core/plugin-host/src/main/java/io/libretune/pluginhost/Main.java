package io.libretune.pluginhost;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintWriter;
import java.util.HashMap;
import java.util.Map;

/**
 * Main entry point for the LibreTune Plugin Host.
 * Communicates with LibreTune via JSON-RPC over stdin/stdout.
 */
public class Main {
    // IMPORTANT: Do NOT use setPrettyPrinting() - Rust reads line-by-line
    private static final Gson gson = new Gson();
    
    private final PluginRegistry registry;
    private final ControllerAccessImpl controllerAccess;
    private final SwingIntrospector introspector;
    private final PrintWriter stdout;
    private volatile boolean running = true;
    
    public Main() {
        this.controllerAccess = new ControllerAccessImpl(this::sendNotification);
        this.registry = new PluginRegistry(controllerAccess);
        this.introspector = new SwingIntrospector();
        this.stdout = new PrintWriter(System.out, true);
    }
    
    public static void main(String[] args) {
        log("LibreTune Plugin Host starting...");
        new Main().run();
    }
    
    private static void log(String message) {
        System.err.println("[PluginHost] " + message);
    }
    
    private static void logError(String message, Throwable e) {
        System.err.println("[PluginHost] ERROR: " + message);
        if (e != null) {
            e.printStackTrace(System.err);
        }
    }
    
    public void run() {
        try (BufferedReader reader = new BufferedReader(new InputStreamReader(System.in))) {
            log("Plugin host ready, waiting for commands...");
            
            String line;
            while (running && (line = reader.readLine()) != null) {
                try {
                    handleMessage(line.trim());
                } catch (Exception e) {
                    logError("Error handling message: " + e.getMessage(), e);
                }
            }
        } catch (Exception e) {
            logError("Fatal error in main loop: " + e.getMessage(), e);
        }
        
        log("Plugin host shutting down...");
        registry.unloadAll();
    }
    
    private void handleMessage(String message) {
        if (message.isEmpty()) return;
        
        JsonObject json = JsonParser.parseString(message).getAsJsonObject();
        
        // Check if it's a request (has id) or notification (no id)
        if (json.has("id")) {
            handleRequest(json);
        } else {
            handleNotification(json);
        }
    }
    
    private void handleRequest(JsonObject request) {
        long id = request.get("id").getAsLong();
        String method = request.get("method").getAsString();
        JsonObject params = request.has("params") ? request.getAsJsonObject("params") : null;
        
        try {
            Object result = dispatch(method, params);
            sendResponse(id, result, null);
        } catch (Exception e) {
            logError("Error executing " + method + ": " + e.getMessage(), e);
            sendResponse(id, null, new RpcError(-32000, e.getMessage()));
        }
    }
    
    private void handleNotification(JsonObject notification) {
        String method = notification.get("method").getAsString();
        JsonObject params = notification.has("params") ? notification.getAsJsonObject("params") : null;
        
        if ("shutdown".equals(method)) {
            running = false;
        } else if ("realtimeUpdate".equals(method)) {
            // Update realtime data from LibreTune
            if (params != null) {
                controllerAccess.updateRealtimeData(params);
            }
        }
    }
    
    private Object dispatch(String method, JsonObject params) throws Exception {
        switch (method) {
            case "loadPlugin":
                return loadPlugin(params);
            case "unloadPlugin":
                return unloadPlugin(params);
            case "listPlugins":
                return listPlugins();
            case "getPluginUi":
                return getPluginUi(params);
            case "pluginEvent":
                return handlePluginEvent(params);
            case "getOutputChannels":
                return controllerAccess.getOutputChannelNames();
            case "getParameters":
                return controllerAccess.getParameterNames();
            case "updateParameter":
                return updateParameter(params);
            default:
                throw new IllegalArgumentException("Unknown method: " + method);
        }
    }
    
    private PluginInfo loadPlugin(JsonObject params) throws Exception {
        String jarPath = params.get("jarPath").getAsString();
        return registry.loadPlugin(jarPath);
    }
    
    private boolean unloadPlugin(JsonObject params) {
        String pluginId = params.get("pluginId").getAsString();
        registry.unloadPlugin(pluginId);
        return true;
    }
    
    private PluginInfo[] listPlugins() {
        return registry.getLoadedPlugins().toArray(new PluginInfo[0]);
    }
    
    private Object getPluginUi(JsonObject params) {
        String pluginId = params.get("pluginId").getAsString();
        var plugin = registry.getPlugin(pluginId);
        if (plugin == null) {
            throw new IllegalArgumentException("Plugin not found: " + pluginId);
        }
        
        var panel = plugin.getPluginPanel();
        if (panel == null) {
            return null;
        }
        
        return introspector.introspect(panel);
    }
    
    private boolean handlePluginEvent(JsonObject params) {
        String pluginId = params.get("pluginId").getAsString();
        JsonObject event = params.getAsJsonObject("event");
        
        var plugin = registry.getPlugin(pluginId);
        if (plugin == null) {
            throw new IllegalArgumentException("Plugin not found: " + pluginId);
        }
        
        // Dispatch event to the appropriate component
        String eventType = event.get("type").getAsString();
        String componentId = event.get("componentId").getAsString();
        
        introspector.dispatchEvent(pluginId, componentId, eventType, event);
        return true;
    }
    
    private boolean updateParameter(JsonObject params) {
        String name = params.get("name").getAsString();
        
        if (params.has("scalarValue")) {
            double value = params.get("scalarValue").getAsDouble();
            sendNotification("parameterUpdate", Map.of(
                "name", name,
                "scalarValue", value
            ));
        } else if (params.has("stringValue")) {
            String value = params.get("stringValue").getAsString();
            sendNotification("parameterUpdate", Map.of(
                "name", name,
                "stringValue", value
            ));
        }
        
        return true;
    }
    
    private void sendResponse(long id, Object result, RpcError error) {
        Map<String, Object> response = new HashMap<>();
        response.put("jsonrpc", "2.0");
        response.put("id", id);
        
        if (error != null) {
            response.put("error", error);
        } else {
            response.put("result", result);
        }
        
        String json = gson.toJson(response);
        synchronized (stdout) {
            stdout.println(json);
        }
    }
    
    private void sendNotification(String method, Object params) {
        Map<String, Object> notification = new HashMap<>();
        notification.put("jsonrpc", "2.0");
        notification.put("method", method);
        if (params != null) {
            notification.put("params", params);
        }
        
        String json = gson.toJson(notification);
        synchronized (stdout) {
            stdout.println(json);
        }
    }
    
    /**
     * Simple RPC error wrapper
     */
    public static class RpcError {
        public final int code;
        public final String message;
        
        public RpcError(int code, String message) {
            this.code = code;
            this.message = message;
        }
    }
}
