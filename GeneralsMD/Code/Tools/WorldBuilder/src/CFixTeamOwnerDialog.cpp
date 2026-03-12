#include "StdAfx.h"
#include "Resource.h"
#include "CFixTeamOwnerDialog.h"
#include "GameLogic/SidesList.h"
#include "Common/WellknownKeys.h"
#include "GameLogic/SidesList.h"

static const char* NEUTRAL_NAME_STR = "(neutral)";

CFixTeamOwnerDialog::CFixTeamOwnerDialog( TeamsInfo *ti, SidesList *sl, UINT nIDTemplate, CWnd* pParentWnd) : CDialog( nIDTemplate, pParentWnd )
{
	m_ti = ti;
	m_sl = sl;
	m_pickedValidTeam = false;
}

AsciiString CFixTeamOwnerDialog::getSelectedOwner()
{
	return m_selectedOwner;
}

BOOL CFixTeamOwnerDialog::OnInitDialog()
{
	AsciiString teamName = "No Name";

	Bool exists;
	AsciiString temp = m_ti->getDict()->getAsciiString(TheKey_teamName, &exists);
	if (exists) {
		teamName = temp;
	}

	CString loadStr;
	loadStr.Format(IDS_REPLACEOWNER, teamName.str());
	CWnd *pWnd = GetDlgItem(IDC_REPLACETHISTEXT);
	pWnd->SetWindowText(loadStr);

	// now load all of the things with the other things
	Int numSides = m_sl->getNumSides();
	CListBox *pList = (CListBox*) GetDlgItem(IDC_VALIDTEAMLIST);

	for (Int i = 0; i < numSides; ++i) {
		SidesInfo *si = m_sl->getSideInfo(i);
		if (!si) {
			continue;
		}

		Bool displayExists;
		AsciiString displayName = si->getDict()->getAsciiString(TheKey_playerDisplayName, &displayExists);
		if (displayExists) {
			if (displayName.isEmpty()) {
				displayName = NEUTRAL_NAME_STR;
			}
			pList->InsertString(-1, displayName.str());
		} else {
			AsciiString internalName = si->getDict()->getAsciiString(TheKey_playerName, &displayExists);
			if (internalName.isEmpty()) {
				internalName = NEUTRAL_NAME_STR;
			}
			pList->InsertString(-1, internalName.str());
		}
	}

	return FALSE;
}

void CFixTeamOwnerDialog::OnOK()
{
	CDialog::OnOK();

	CListBox *pList = (CListBox*) GetDlgItem(IDC_VALIDTEAMLIST);
	int curSel = pList->GetCurSel();

	if (curSel < 0) { 
		return;
	}

	SidesInfo *si = m_sl->getSideInfo(curSel);
	if (!si) {
		return;
	}

	m_pickedValidTeam = true;
	Bool exists;
	m_selectedOwner = si->getDict()->getAsciiString(TheKey_playerName, &exists);
}

BEGIN_MESSAGE_MAP(CFixTeamOwnerDialog, CDialog)
END_MESSAGE_MAP()
