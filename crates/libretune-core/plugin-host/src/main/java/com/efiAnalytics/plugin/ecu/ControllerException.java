package com.efiAnalytics.plugin.ecu;

/**
 * Exception thrown when ECU controller operations fail.
 * 
 * This is a LibreTune open-source implementation of a documented
 * plugin API interface. Not derived from proprietary code.
 */
public class ControllerException extends Exception {
    public ControllerException() {
        super();
    }
    
    public ControllerException(String message) {
        super(message);
    }
    
    public ControllerException(String message, Throwable cause) {
        super(message, cause);
    }
    
    public ControllerException(Throwable cause) {
        super(cause);
    }
}
