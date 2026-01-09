package com.efiAnalytics.plugin.ecu;

/**
 * Exception for math/expression evaluation errors in plugin API.
 */
public class MathException extends Exception {
    public MathException() {
        super();
    }
    
    public MathException(String message) {
        super(message);
    }
    
    public MathException(String message, Throwable cause) {
        super(message, cause);
    }
    
    public MathException(Throwable cause) {
        super(cause);
    }
}
