#ifndef MODELSELECTORWIDGET_H
#define MODELSELECTORWIDGET_H

#include <QWidget>

class QComboBox;
class QLabel;
class QProgressBar;
class QPushButton;

class ModelSelectorWidget : public QWidget {
  Q_OBJECT
public:
  explicit ModelSelectorWidget(QWidget *parent = nullptr);
  ~ModelSelectorWidget() override = default;

  void refreshModelList();

signals:
  void modelChanged(const QString &modelId);
  void downloadRequested(const QString &modelId);

private slots:
  void onModelSelected(int index);
  void onDownloadClicked();
  void onDeleteClicked();

private:
  void updateStatusDisplay();

  QComboBox *m_modelCombo = nullptr;
  QLabel *m_statusLabel = nullptr;
  QLabel *m_descriptionLabel = nullptr;
  QLabel *m_sizeLabel = nullptr;
  QProgressBar *m_progressBar = nullptr;
  QPushButton *m_downloadButton = nullptr;
  QPushButton *m_deleteButton = nullptr;
  QString m_currentModelId;
  bool m_downloading = false;
};

#endif // MODELSELECTORWIDGET_H
