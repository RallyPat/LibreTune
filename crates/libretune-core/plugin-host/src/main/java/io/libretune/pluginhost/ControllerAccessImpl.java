package io.libretune.pluginhost;

import com.efiAnalytics.plugin.ecu.ControllerAccess;
import com.efiAnalytics.plugin.ecu.ControllerException;
import com.efiAnalytics.plugin.ecu.MathException;
import com.efiAnalytics.plugin.ecu.servers.ControllerParameterServer;
import com.efiAnalytics.plugin.ecu.servers.OutputChannelServer;
import com.efiAnalytics.plugin.ecu.servers.UiSettingServer;
import com.google.gson.JsonObject;

import java.util.*;
import java.util.function.BiConsumer;

/**
 * LibreTune implementation of ControllerAccess for plugin compatibility.
 */
public class ControllerAccessImpl extends ControllerAccess {
    private static void log(String message) {
        System.err.println("[ControllerAccess] " + message);
    }
    
    private static ControllerAccessImpl instance;
    
    private final OutputChannelServerImpl outputChannelServer;
    private final ControllerParameterServerImpl parameterServer;
    private final Map<String, UiSettingServerImpl> uiSettingServers = new HashMap<>();
    private final BiConsumer<String, Object> notificationSender;
    
    // Cached realtime data from LibreTune
    private final Map<String, Double> realtimeData = new HashMap<>();
    
    // ECU configuration name (usually just one)
    private String ecuConfigName = "default";
    
    public ControllerAccessImpl(BiConsumer<String, Object> notificationSender) {
        super();
        this.notificationSender = notificationSender;
        this.outputChannelServer = new OutputChannelServerImpl(this);
        this.parameterServer = new ControllerParameterServerImpl(this, notificationSender);
        instance = this;
        setInstance(this);
    }
    
    /**
     * Get singleton instance.
     */
    public static ControllerAccess getInstance() {
        return instance;
    }
    
    @Override
    public OutputChannelServer getOutputChannelServer() {
        return outputChannelServer;
    }
    
    @Override
    public ControllerParameterServer getControllerParameterServer() {
        return parameterServer;
    }
    
    @Override
    public UiSettingServer getUiComponentServer(String ecuConfigName) {
        return uiSettingServers.computeIfAbsent(ecuConfigName, 
            name -> new UiSettingServerImpl(name, notificationSender));
    }

    @Override
    public String[] getEcuConfigurationNames() {
        return new String[] { ecuConfigName };
    }
    
    @Override
    public double evaluateExpression(String ecuConfigName, String expression) throws MathException {
        // Simple expression evaluation - just return channel value if it's a simple reference
        if (realtimeData.containsKey(expression)) {
            return realtimeData.get(expression);
        }
        
        // For complex expressions, we'd need a proper expression parser
        log("WARN: Complex expression evaluation not implemented: " + expression);
        return 0.0;
    }
    
    @Override
    public void sendBurnCommand(String ecuConfigName) throws ControllerException {
        log("Burn command requested for: " + ecuConfigName);
        notificationSender.accept("burnCommand", Map.of("ecuConfigName", ecuConfigName));
    }
    
    @Override
    public boolean isOnline() {
        return true; // LibreTune manages connection state
    }
    
    /**
     * Update realtime data from LibreTune.
     */
    public void updateRealtimeData(JsonObject data) {
        for (String key : data.keySet()) {
            if (data.get(key).isJsonPrimitive()) {
                try {
                    realtimeData.put(key, data.get(key).getAsDouble());
                } catch (Exception e) {
                    // Ignore non-numeric values
                }
            }
        }
        
        // Notify subscribers
        outputChannelServer.notifySubscribers(realtimeData);
    }
    
    /**
     * Get current realtime value for a channel.
     */
    public Double getRealtimeValue(String channelName) {
        return realtimeData.get(channelName);
    }
    
    /**
     * Get all output channel names.
     */
    public Set<String> getOutputChannelNames() {
        return new HashSet<>(realtimeData.keySet());
    }
    
    /**
     * Get all parameter names (requests from LibreTune).
     */
    public Set<String> getParameterNames() {
        return parameterServer.getKnownParameters();
    }
    
    /**
     * Set ECU configuration name.
     */
    public void setEcuConfigName(String name) {
        this.ecuConfigName = name;
    }
}
