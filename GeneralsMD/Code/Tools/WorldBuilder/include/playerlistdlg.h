#if !defined(AFX_PLAYERLISTDLG_H__103B4125_78ED_48A8_9DBB_289DDC6B0208__INCLUDED_)
#define AFX_PLAYERLISTDLG_H__103B4125_78ED_48A8_9DBB_289DDC6B0208__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// playerlistdlg.h : header file
//

#include "GameLogic/SidesList.h"
#include "CButtonShowColor.h"

/////////////////////////////////////////////////////////////////////////////
// PlayerListDlg dialog

class PlayerListDlg : public CDialog
{
// Construction
public:
	PlayerListDlg(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(PlayerListDlg)
	enum { IDD = IDD_PLAYERLIST };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(PlayerListDlg)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
protected:

	Int					m_updating;
	SidesList			m_sides;
	Int					m_curPlayerIdx;
	CButtonShowColor	m_colorButton;

	void updateTheUI(void);
	void PopulateColorComboBox(void);
	void SelectColor(RGBColor rgb);

	// Generated message map functions
	//{{AFX_MSG(PlayerListDlg)
	afx_msg void OnNewplayer();
	afx_msg void OnEditplayer();
	afx_msg void OnRemoveplayer();
	afx_msg void OnSelchangePlayers();
	virtual BOOL OnInitDialog();
	afx_msg void OnDblclkPlayers();
	afx_msg void OnSelchangeAllieslist();
	afx_msg void OnSelchangeEnemieslist();
	virtual void OnOK();
	virtual void OnCancel();
	afx_msg void OnPlayeriscomputer();
	afx_msg void OnEditchangePlayerfaction();
	afx_msg void OnChangePlayername();
	afx_msg void OnChangePlayerdisplayname();
	afx_msg void OnColorPress();
	afx_msg void OnSelectPlayerColor();
	afx_msg void OnAddskirmishplayers();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_PLAYERLISTDLG_H__103B4125_78ED_48A8_9DBB_289DDC6B0208__INCLUDED_)
