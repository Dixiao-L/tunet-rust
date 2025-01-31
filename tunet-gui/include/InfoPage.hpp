#pragma once

#include <FluxCircle.hpp>
#include <Model.hpp>
#include <QComboBox>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QProgressIndicator.hpp>
#include <QPushButton>
#include <QVBoxLayout>
#include <QWidget>

namespace TUNet
{
    struct InfoPage : QWidget
    {
    public:
        InfoPage(QWidget* parent, Model* pmodel);
        ~InfoPage() override;

        void spawn_login();
        void spawn_logout();
        void spawn_flux();

        void update_state();
        void update_state_back(int index);
        void update_log();
        void update_flux();
        void update_log_busy();

    private:
        Model* m_pmodel{};

        QVBoxLayout m_root_layout{ this };

        // basic info
        QGridLayout m_info_layout{};

        FluxCircle m_flux_circle{};

        QProgressIndicator m_log_busy_indicator{};

        QWidget m_flux_widget{};
        QVBoxLayout m_flux_layout{ &m_flux_widget };
        QLabel m_username_label{};
        QLabel m_flux_label{};
        QLabel m_online_time_label{};
        QLabel m_balance_label{};

        // state
        QHBoxLayout m_state_layout{};
        QLabel m_state_label{};
        QComboBox m_state_combo{};

        // log
        QLabel m_log_label{};

        // command
        QHBoxLayout m_command_layout{};
        QPushButton m_login_button{};
        QPushButton m_logout_button{};
        QPushButton m_flux_button{};
    };
} // namespace TUNet
