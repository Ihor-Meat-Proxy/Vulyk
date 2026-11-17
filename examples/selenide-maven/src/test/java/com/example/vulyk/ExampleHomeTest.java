package com.example.vulyk;

import static com.codeborne.selenide.Condition.visible;
import static com.codeborne.selenide.Selenide.$;
import static com.codeborne.selenide.Selenide.open;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

/**
 * Demo Selenide test. The {@code @DisplayName} embeds {@code KOC-12} so Vulyk
 * maps the Surefire {@code <testcase>} without a custom reporter. Prefer a
 * {@code test_key} / {@code case_key} property in real suites when titles must
 * stay free of keys — see {@link VulykCaseKeyExtension}.
 */
@ExtendWith(VulykCaseKeyExtension.class)
class ExampleHomeTest {

  @Test
  @DisplayName("KOC-12 Demo heading is visible")
  @VulykCaseKey("KOC-12")
  void demoHeadingVisible() {
    // data: URL keeps the sample offline-friendly in CI sandboxes.
    open("data:text/html,<html><body><h1>Vulyk demo</h1></body></html>");
    $("h1").shouldBe(visible);
  }
}
