package io.libretune.pluginhost;

import com.efiAnalytics.plugin.ecu.ControllerException;
import com.efiAnalytics.plugin.ecu.OutputChannel;
import com.efiAnalytics.plugin.ecu.OutputChannelClient;
import com.efiAnalytics.plugin.ecu.servers.OutputChannelServer;

import java.util.*;

/**
 * Implementation of OutputChannelServer for LibreTune plugin compatibility.
 */
public class OutputChannelServerImpl implements OutputChannelServer {
    private static void log(String message) {
        System.err.println("[OutputChannelServer] " + message);
    }
    
    private static void logError(String message) {
        System.err.println("[OutputChannelServer] ERROR: " + message);
    }
    
    private final ControllerAccessImpl controllerAccess;
    private final Map<String, List<OutputChannelClient>> subscriptions = new HashMap<>();
    private final Map<String, OutputChannel> channelCache = new HashMap<>();
    
    public OutputChannelServerImpl(ControllerAccessImpl controllerAccess) {
        this.controllerAccess = controllerAccess;
    }
    
    @Override
    public void subscribe(String ecuConfigName, String outputChannelName, OutputChannelClient client) 
            throws ControllerException {
        log("Subscribe to channel: " + outputChannelName + " for " + ecuConfigName);
        subscriptions.computeIfAbsent(outputChannelName, k -> new ArrayList<>()).add(client);
    }
    
    @Override
    public void unsubscribeConfiguration(String ecuConfigName) {
        // Remove all subscriptions (we only support one config currently)
        subscriptions.clear();
    }
    
    @Override
    public void unsubscribe(OutputChannelClient client) {
        for (List<OutputChannelClient> clients : subscriptions.values()) {
            clients.remove(client);
        }
    }
    
    @Override
    public String[] getOutputChannels(String ecuConfigName) throws ControllerException {
        return controllerAccess.getOutputChannelNames().toArray(new String[0]);
    }
    
    @Override
    public OutputChannel getOutputChannel(String ecuConfigName, String outputChannelName) 
            throws ControllerException {
        OutputChannel channel = channelCache.get(outputChannelName);
        if (channel == null) {
            channel = new OutputChannel();
            channel.setName(outputChannelName);
            channel.setUnits("");
            channel.setMinValue(0);
            channel.setMaxValue(100);
            channelCache.put(outputChannelName, channel);
        }
        return channel;
    }
    
    /**
     * Notify subscribers of updated realtime data.
     */
    public void notifySubscribers(Map<String, Double> data) {
        for (Map.Entry<String, List<OutputChannelClient>> entry : subscriptions.entrySet()) {
            String channelName = entry.getKey();
            Double value = data.get(channelName);
            
            if (value != null) {
                for (OutputChannelClient client : entry.getValue()) {
                    try {
                        client.setCurrentOutputChannelValue(channelName, value);
                    } catch (Exception e) {
                        logError("Error notifying subscriber for " + channelName + ": " + e.getMessage());
                    }
                }
            }
        }
    }
}
