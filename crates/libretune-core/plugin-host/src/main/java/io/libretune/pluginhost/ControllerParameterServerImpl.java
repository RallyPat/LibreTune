package io.libretune.pluginhost;

import com.efiAnalytics.plugin.ecu.ControllerException;
import com.efiAnalytics.plugin.ecu.ControllerParameter;
import com.efiAnalytics.plugin.ecu.ControllerParameterChangeListener;
import com.efiAnalytics.plugin.ecu.servers.ControllerParameterServer;

import java.util.*;
import java.util.function.BiConsumer;

/**
 * Implementation of TunerStudio's ControllerParameterServer.
 */
public class ControllerParameterServerImpl implements ControllerParameterServer {
    private static void log(String message) {
        System.err.println("[ParameterServer] " + message);
    }
    
    private static void logError(String message) {
        System.err.println("[ParameterServer] ERROR: " + message);
    }
    
    private final ControllerAccessImpl controllerAccess;
    private final BiConsumer<String, Object> notificationSender;
    private final Map<String, List<ControllerParameterChangeListener>> subscriptions = new HashMap<>();
    private final Map<String, ControllerParameter> parameterCache = new HashMap<>();
    private final Set<String> knownParameters = new HashSet<>();
    
    public ControllerParameterServerImpl(ControllerAccessImpl controllerAccess, 
                                          BiConsumer<String, Object> notificationSender) {
        this.controllerAccess = controllerAccess;
        this.notificationSender = notificationSender;
    }
    
    @Override
    public void subscribe(String ecuConfigName, String parameterName, 
                         ControllerParameterChangeListener listener) {
        log("Subscribe to parameter: " + parameterName + " for " + ecuConfigName);
        subscriptions.computeIfAbsent(parameterName, k -> new ArrayList<>()).add(listener);
        knownParameters.add(parameterName);
    }
    
    @Override
    public void unsubscribe(ControllerParameterChangeListener listener) {
        for (List<ControllerParameterChangeListener> listeners : subscriptions.values()) {
            listeners.remove(listener);
        }
    }
    
    @Override
    public String[] getParameterNames(String ecuConfigName) {
        return knownParameters.toArray(new String[0]);
    }
    
    @Override
    public ControllerParameter getControllerParameter(String ecuConfigName, String parameterName) {
        ControllerParameter param = parameterCache.get(parameterName);
        if (param == null) {
            // Request parameter info from LibreTune
            param = new ControllerParameter();
            // Default to scalar type
            param.setParamClass(ControllerParameter.PARAM_CLASS_SCALAR);
            param.setUnits("");
            param.setMin(0);
            param.setMax(100);
            param.setDecimalPlaces(2);
            parameterCache.put(parameterName, param);
            knownParameters.add(parameterName);
        }
        return param;
    }
    
    @Override
    public void updateParameter(String ecuConfigName, String parameterName, double value) {
        log("Update parameter " + parameterName + " = " + value);
        
        // Send update to LibreTune
        notificationSender.accept("parameterUpdate", Map.of(
            "name", parameterName,
            "scalarValue", value
        ));
        
        // Update cache and notify listeners
        ControllerParameter param = getControllerParameter(ecuConfigName, parameterName);
        param.setScalarValue(value);
        notifyListeners(parameterName, param);
    }
    
    @Override
    public void updateParameter(String ecuConfigName, String parameterName, double[][] values) {
        int rows = values.length;
        int cols = rows > 0 ? values[0].length : 0;
        log("Update array parameter " + parameterName + " = [" + rows + "x" + cols + "]");
        
        // Send update to LibreTune
        notificationSender.accept("parameterUpdate", Map.of(
            "name", parameterName,
            "arrayValues", values
        ));
        
        // Update cache and notify listeners
        ControllerParameter param = getControllerParameter(ecuConfigName, parameterName);
        param.setArrayValues(values);
        notifyListeners(parameterName, param);
    }
    
    @Override
    public void updateParameter(String ecuConfigName, String parameterName, String value) {
        log("Update bits parameter " + parameterName + " = " + value);
        
        // Send update to LibreTune
        notificationSender.accept("parameterUpdate", Map.of(
            "name", parameterName,
            "stringValue", value
        ));
        
        // Update cache and notify listeners
        ControllerParameter param = getControllerParameter(ecuConfigName, parameterName);
        param.setStringValue(value);
        notifyListeners(parameterName, param);
    }
    
    @Override
    public void burnData(String ecuConfigName) throws ControllerException {
        log("Burn data requested for: " + ecuConfigName);
        controllerAccess.sendBurnCommand(ecuConfigName);
    }
    
    /**
     * Update parameter from LibreTune and notify listeners.
     */
    public void updateFromLibreTune(String parameterName, ControllerParameter param) {
        parameterCache.put(parameterName, param);
        notifyListeners(parameterName, param);
    }
    
    /**
     * Get known parameter names.
     */
    public Set<String> getKnownParameters() {
        return new HashSet<>(knownParameters);
    }
    
    private void notifyListeners(String parameterName, ControllerParameter param) {
        List<ControllerParameterChangeListener> listeners = subscriptions.get(parameterName);
        if (listeners != null) {
            for (ControllerParameterChangeListener listener : listeners) {
                try {
                    listener.controllerParameterChanged(parameterName, param);
                } catch (Exception e) {
                    logError("Error notifying listener for " + parameterName + ": " + e.getMessage());
                }
            }
        }
    }
}
