#include "MainWindow.h"

#include <QApplication>
#include <QCommandLineOption>
#include <QCommandLineParser>
#include <QDebug>
#include <QTimer>

#include <cstdio>

extern "C" {
#include "toaster.h"
}

int main(int argc, char *argv[])
{
  QApplication app(argc, argv);
  QCommandLineParser parser;
  bool started = toaster_startup();
  bool automationRequested;

  if (!started)
    return 1;

  parser.setApplicationDescription("Toaster");
  parser.addHelpOption();

  QCommandLineOption automationMediaOption("automation-media",
                                           "Open media, run the built-in workflow, and quit.",
                                           "path");
  QCommandLineOption automationProjectOption("automation-project",
                                             "Save automation project to this path.", "path");
  QCommandLineOption automationExportOption("automation-export",
                                            "Export automation media to this path.", "path");

  parser.addOption(automationMediaOption);
  parser.addOption(automationProjectOption);
  parser.addOption(automationExportOption);
  parser.process(app);

  automationRequested = parser.isSet(automationMediaOption) || parser.isSet(automationProjectOption) ||
                        parser.isSet(automationExportOption);
  if (automationRequested &&
      (!parser.isSet(automationMediaOption) || !parser.isSet(automationProjectOption) ||
       (parser.isSet(automationExportOption) && !parser.isSet(automationProjectOption)))) {
    qCritical().noquote()
      << "Automation mode requires --automation-media and --automation-project. "
         "--automation-export is optional and enables the full edit/export workflow.";
    toaster_shutdown();
    return 2;
  }

  MainWindow window;
  window.show();

  if (automationRequested) {
    QString mediaPath = parser.value(automationMediaOption);
    QString projectPath = parser.value(automationProjectOption);
    QString exportPath = parser.value(automationExportOption);

    QTimer::singleShot(0, &app, [&window, &app, mediaPath, projectPath, exportPath]() {
      QString errorMessage;
      bool ok = exportPath.isEmpty()
                  ? window.runTranscriptionAutomation(mediaPath, projectPath, &errorMessage)
                  : window.runAutomationWorkflow(mediaPath, projectPath, exportPath, &errorMessage);

      if (!ok) {
        QByteArray errorBytes = errorMessage.toUtf8();
        if (!errorBytes.isEmpty())
          fprintf(stderr, "%s\n", errorBytes.constData());
      }

      app.exit(ok ? 0 : 3);
    });
  }

  int result = app.exec();
  toaster_shutdown();
  return result;
}
