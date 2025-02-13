#include <InfoPage.hpp>

namespace TUNet
{
    InfoPage::InfoPage(QWidget* parent, Model* pmodel) : QWidget(parent), m_pmodel(pmodel)
    {
        m_flux_layout.addWidget(&m_username_label, Qt::AlignLeft);
        m_flux_layout.addWidget(&m_flux_label, Qt::AlignLeft);
        m_flux_layout.addWidget(&m_online_time_label, Qt::AlignLeft);
        m_flux_layout.addWidget(&m_balance_label, Qt::AlignLeft);

        m_flux_circle.set_color(m_pmodel->accent_color());
        m_info_layout.addWidget(&m_flux_circle, 0, 0);
        m_info_layout.addWidget(&m_flux_widget, 0, 0, Qt::AlignCenter);
        m_log_busy_indicator.setColor(m_pmodel->accent_color());
        m_info_layout.addWidget(&m_log_busy_indicator, 0, 0, Qt::AlignCenter);
        m_root_layout.addLayout(&m_info_layout, 1);

        m_state_layout.addStretch();
        m_state_label.setText(QStringLiteral(u"连接方式："));
        m_state_layout.addWidget(&m_state_label);
        m_state_combo.addItem(QStringLiteral(u"Net"));
        m_state_combo.addItem(QStringLiteral(u"Auth4"));
        m_state_combo.addItem(QStringLiteral(u"Auth6"));
        QObject::connect(&m_state_combo, static_cast<void (QComboBox::*)(int)>(&QComboBox::currentIndexChanged), this, &InfoPage::update_state_back);
        m_state_layout.addWidget(&m_state_combo);
        m_state_layout.addStretch();
        m_root_layout.addLayout(&m_state_layout);

        m_log_label.setTextInteractionFlags(Qt::TextSelectableByMouse);
        m_log_label.setWordWrap(true);
        m_log_label.setAlignment(Qt::AlignCenter);
        m_root_layout.addWidget(&m_log_label);

        m_login_button.setText(QStringLiteral(u"登录"));
        QObject::connect(&m_login_button, &QPushButton::clicked, this, &InfoPage::spawn_login);
        m_logout_button.setText(QStringLiteral(u"注销"));
        QObject::connect(&m_logout_button, &QPushButton::clicked, this, &InfoPage::spawn_logout);
        m_flux_button.setText(QStringLiteral(u"刷新"));
        QObject::connect(&m_flux_button, &QPushButton::clicked, this, &InfoPage::spawn_flux);

        m_command_layout.addWidget(&m_login_button);
        m_command_layout.addWidget(&m_logout_button);
        m_command_layout.addWidget(&m_flux_button);
        m_root_layout.addLayout(&m_command_layout, 1);

        QObject::connect(m_pmodel, &Model::state_changed, this, &InfoPage::update_state);
        QObject::connect(m_pmodel, &Model::log_changed, this, &InfoPage::update_log);
        QObject::connect(m_pmodel, &Model::flux_changed, this, &InfoPage::update_flux);
        QObject::connect(m_pmodel, &Model::log_busy_changed, this, &InfoPage::update_log_busy);
    }

    InfoPage::~InfoPage() {}

    void InfoPage::spawn_login()
    {
        m_pmodel->queue(Action::Login);
    }

    void InfoPage::spawn_logout()
    {
        m_pmodel->queue(Action::Logout);
    }

    void InfoPage::spawn_flux()
    {
        m_pmodel->queue(Action::Flux);
    }

    void InfoPage::update_state()
    {
        auto state = m_pmodel->state();
        m_state_combo.setCurrentIndex(static_cast<int>(state) - 1);
        m_pmodel->queue(Action::Flux);
    }

    void InfoPage::update_state_back(int index)
    {
        m_pmodel->queue_state(static_cast<State>(index + 1));
    }

    void InfoPage::update_log()
    {
        m_log_label.setText(m_pmodel->log());
    }

    void InfoPage::update_flux()
    {
        auto flux = m_pmodel->flux();
        m_username_label.setText(QStringLiteral(u"用户：%1").arg(flux.username));
        m_flux_label.setText(QStringLiteral(u"流量：%1").arg(flux.flux.toString()));
        m_online_time_label.setText(QStringLiteral(u"时长：%1").arg(format_duration(flux.online_time)));
        m_balance_label.setText(QStringLiteral(u"余额：￥%1").arg(flux.balance, 0, 'f', 2));
        m_flux_circle.update_flux(flux.flux, flux.balance);
    }

    void InfoPage::update_log_busy()
    {
        bool free = !m_pmodel->log_busy();
        m_login_button.setEnabled(free);
        m_logout_button.setEnabled(free);
        m_flux_button.setEnabled(free);
        if (!free)
        {
            m_log_busy_indicator.startAnimation();
        }
        else
        {
            m_log_busy_indicator.stopAnimation();
        }
    }
} // namespace TUNet
