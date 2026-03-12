

#include "Common/AsciiString.h"
class TeamsInfo;
class SidesList;

#pragma once

class CFixTeamOwnerDialog : public CDialog
{
	public:
		enum { IDD = IDD_CHANGE_TEAM_OWNER };
		CFixTeamOwnerDialog( TeamsInfo *ti, SidesList *sideList, UINT nIDTemplate = CFixTeamOwnerDialog::IDD,  CWnd* pParentWnd = NULL );
		AsciiString getSelectedOwner();
		Bool pickedValidTeam() { return m_pickedValidTeam; }

	protected:
		virtual BOOL OnInitDialog();
		afx_msg void OnOK();
		DECLARE_MESSAGE_MAP()

	protected:
		Bool m_pickedValidTeam;
		AsciiString m_selectedOwner;
		TeamsInfo *m_ti;
		SidesList *m_sl;
};