#include "ModelSelectorWidget.h"

#include <QComboBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QMessageBox>
#include <QProgressBar>
#include <QPushButton>
#include <QThread>
#include <QVBoxLayout>

extern "C" {
#include "toaster.h"
}

/* Worker thread for model download */
class DownloadWorker : public QThread {
  Q_OBJECT
public:
  explicit DownloadWorker(const QString &modelId, QObject *parent = nullptr)
      : QThread(parent), m_modelId(modelId)
  {
  }

signals:
  void progressUpdated(const QString &modelId, quint64 downloaded, quint64 total);
  void finished(bool success);

protected:
  void run() override
  {
    bool ok = toaster_model_download(
        m_modelId.toUtf8().constData(),
        [](const char *model_id, uint64_t downloaded, uint64_t total, void *user_data) {
          auto *worker = static_cast<DownloadWorker *>(user_data);
          emit worker->progressUpdated(QString::fromUtf8(model_id), downloaded, total);
        },
        this);
    emit finished(ok);
  }

private:
  QString m_modelId;
};

ModelSelectorWidget::ModelSelectorWidget(QWidget *parent) : QWidget(parent)
{
  auto *layout = new QVBoxLayout(this);
  layout->setContentsMargins(8, 8, 8, 8);
  layout->setSpacing(6);

  /* Title */
  auto *titleLabel = new QLabel(QStringLiteral("🎤 Transcription Model"));
  titleLabel->setStyleSheet("font-weight: bold; font-size: 13px;");
  layout->addWidget(titleLabel);

  /* Model selector combo */
  m_modelCombo = new QComboBox;
  m_modelCombo->setMinimumWidth(200);
  layout->addWidget(m_modelCombo);

  /* Description */
  m_descriptionLabel = new QLabel;
  m_descriptionLabel->setWordWrap(true);
  m_descriptionLabel->setStyleSheet("color: #888; font-size: 11px;");
  layout->addWidget(m_descriptionLabel);

  /* Size and status row */
  auto *statusRow = new QHBoxLayout;
  m_sizeLabel = new QLabel;
  m_sizeLabel->setStyleSheet("font-size: 11px;");
  statusRow->addWidget(m_sizeLabel);

  m_statusLabel = new QLabel;
  m_statusLabel->setStyleSheet("font-size: 11px; font-weight: bold;");
  statusRow->addWidget(m_statusLabel);
  statusRow->addStretch();
  layout->addLayout(statusRow);

  /* Progress bar (hidden by default) */
  m_progressBar = new QProgressBar;
  m_progressBar->setRange(0, 100);
  m_progressBar->setVisible(false);
  m_progressBar->setTextVisible(true);
  m_progressBar->setFixedHeight(18);
  layout->addWidget(m_progressBar);

  /* Action buttons */
  auto *buttonRow = new QHBoxLayout;
  m_downloadButton = new QPushButton(QStringLiteral("⬇ Download"));
  m_downloadButton->setToolTip("Download this model from HuggingFace");
  buttonRow->addWidget(m_downloadButton);

  m_deleteButton = new QPushButton(QStringLiteral("🗑 Delete"));
  m_deleteButton->setToolTip("Remove downloaded model file");
  buttonRow->addWidget(m_deleteButton);

  buttonRow->addStretch();
  layout->addLayout(buttonRow);

  layout->addStretch();

  /* Connections */
  connect(m_modelCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this,
          &ModelSelectorWidget::onModelSelected);
  connect(m_downloadButton, &QPushButton::clicked, this, &ModelSelectorWidget::onDownloadClicked);
  connect(m_deleteButton, &QPushButton::clicked, this, &ModelSelectorWidget::onDeleteClicked);

  refreshModelList();
}

void ModelSelectorWidget::refreshModelList()
{
  m_modelCombo->blockSignals(true);
  m_modelCombo->clear();

  size_t count = toaster_model_catalog_count();
  const char *active = toaster_model_get_active();
  int activeIndex = 0;

  for (size_t i = 0; i < count; i++) {
    toaster_model_info_t info;
    if (!toaster_model_catalog_get(i, &info))
      continue;

    QString label = QString::fromUtf8(info.name);
    if (info.is_downloaded)
      label = QStringLiteral("✅ ") + label;
    if (info.is_recommended)
      label += QStringLiteral(" ⭐");

    m_modelCombo->addItem(label, QString::fromUtf8(info.id));

    if (active && strcmp(info.id, active) == 0)
      activeIndex = static_cast<int>(i);
  }

  m_modelCombo->setCurrentIndex(activeIndex);
  m_modelCombo->blockSignals(false);

  onModelSelected(activeIndex);
}

void ModelSelectorWidget::onModelSelected(int index)
{
  if (index < 0)
    return;

  m_currentModelId = m_modelCombo->itemData(index).toString();
  toaster_model_set_active(m_currentModelId.toUtf8().constData());

  updateStatusDisplay();
  emit modelChanged(m_currentModelId);
}

void ModelSelectorWidget::updateStatusDisplay()
{
  toaster_model_info_t info;
  if (!toaster_model_catalog_find(m_currentModelId.toUtf8().constData(), &info)) {
    m_descriptionLabel->clear();
    m_sizeLabel->clear();
    m_statusLabel->clear();
    return;
  }

  m_descriptionLabel->setText(QString::fromUtf8(info.description));
  m_sizeLabel->setText(
      QStringLiteral("%1 MB · %2 language%3")
          .arg(info.size_mb)
          .arg(info.language_count)
          .arg(info.language_count > 1 ? "s" : ""));

  if (info.is_downloaded) {
    m_statusLabel->setText(QStringLiteral("● Ready"));
    m_statusLabel->setStyleSheet("color: #4CAF50; font-size: 11px; font-weight: bold;");
    m_downloadButton->setEnabled(false);
    m_downloadButton->setText(QStringLiteral("✅ Downloaded"));
    m_deleteButton->setEnabled(true);
  } else {
    m_statusLabel->setText(QStringLiteral("○ Not downloaded"));
    m_statusLabel->setStyleSheet("color: #FF9800; font-size: 11px; font-weight: bold;");
    m_downloadButton->setEnabled(!m_downloading);
    m_downloadButton->setText(QStringLiteral("⬇ Download"));
    m_deleteButton->setEnabled(false);
  }
}

void ModelSelectorWidget::onDownloadClicked()
{
  if (m_downloading || m_currentModelId.isEmpty())
    return;

  m_downloading = true;
  m_downloadButton->setEnabled(false);
  m_downloadButton->setText(QStringLiteral("⏳ Downloading..."));
  m_progressBar->setVisible(true);
  m_progressBar->setValue(0);
  m_deleteButton->setEnabled(false);

  auto *worker = new DownloadWorker(m_currentModelId, this);

  connect(worker, &DownloadWorker::progressUpdated, this,
          [this](const QString &, quint64 downloaded, quint64 total) {
            if (total > 0) {
              int pct = static_cast<int>((downloaded * 100) / total);
              m_progressBar->setValue(pct);
              m_progressBar->setFormat(
                  QStringLiteral("%1 / %2 MB").arg(downloaded / (1024 * 1024)).arg(total / (1024 * 1024)));
            }
          });

  connect(worker, &DownloadWorker::finished, this, [this, worker](bool success) {
    m_downloading = false;
    m_progressBar->setVisible(false);
    worker->deleteLater();

    if (success) {
      toaster_model_refresh_status();
      refreshModelList();
    } else {
      m_downloadButton->setEnabled(true);
      m_downloadButton->setText(QStringLiteral("⬇ Download"));
      QMessageBox::warning(this, QStringLiteral("Download Failed"),
                           QStringLiteral("Failed to download model. Check your internet connection."));
    }
  });

  worker->start();
  emit downloadRequested(m_currentModelId);
}

void ModelSelectorWidget::onDeleteClicked()
{
  if (m_currentModelId.isEmpty())
    return;

  auto reply = QMessageBox::question(
      this, QStringLiteral("Delete Model"),
      QStringLiteral("Delete the downloaded model file?\nYou can re-download it later."),
      QMessageBox::Yes | QMessageBox::No);

  if (reply != QMessageBox::Yes)
    return;

  if (toaster_model_delete(m_currentModelId.toUtf8().constData())) {
    toaster_model_refresh_status();
    refreshModelList();
  }
}

#include "ModelSelectorWidget.moc"
