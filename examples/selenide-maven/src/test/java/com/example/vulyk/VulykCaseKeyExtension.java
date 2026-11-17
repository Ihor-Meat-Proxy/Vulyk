package com.example.vulyk;

import org.junit.jupiter.api.extension.BeforeEachCallback;
import org.junit.jupiter.api.extension.ExtensionContext;
import org.junit.jupiter.api.extension.TestWatcher;
import org.junit.platform.commons.support.AnnotationSupport;

/**
 * Publishes {@code case_key} / {@code test_key} into the JUnit report so Vulyk
 * can map results even when the display name does not contain the key.
 *
 * <p>Surefire records {@link org.junit.jupiter.api.extension.ExtensionContext.Store}
 * report entries as {@code <property>} nodes when using a recent Surefire +
 * JUnit Jupiter combination; the display-name fallback remains the safest path.
 */
public class VulykCaseKeyExtension implements BeforeEachCallback, TestWatcher {

  @Override
  public void beforeEach(ExtensionContext context) {
    AnnotationSupport.findAnnotation(context.getElement(), VulykCaseKey.class)
        .or(() -> AnnotationSupport.findAnnotation(context.getRequiredTestClass(), VulykCaseKey.class))
        .ifPresent(
            ann -> {
              context.publishReportEntry("case_key", ann.value());
              context.publishReportEntry("test_key", ann.value());
            });
  }
}
