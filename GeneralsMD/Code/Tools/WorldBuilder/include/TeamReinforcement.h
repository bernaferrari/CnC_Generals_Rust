#if !defined(AFX_TEAMREINFORCEMENT_H__29B5C0C7_10E8_4869_8B43_815422C51C24__INCLUDED_)
#define AFX_TEAMREINFORCEMENT_H__29B5C0C7_10E8_4869_8B43_815422C51C24__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// TeamReinforcement.h : header file
//

/////////////////////////////////////////////////////////////////////////////
// TeamReinforcement dialog

class TeamReinforcement : public CPropertyPage
{
// Construction
public:
	TeamReinforcement();   // standard constructor

// Dialog Data
	//{{AFX_DATA(TeamReinforcement)
	enum { IDD = IDD_TeamReinforcement };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(TeamReinforcement)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
protected:
	Dict *m_teamDict;
public:
	void setTeamDict(Dict *pDict) {m_teamDict = pDict;};
protected:

	// Generated message map functions
	//{{AFX_MSG(TeamReinforcement)
	afx_msg void OnDeployBy();
	afx_msg void OnTeamStartsFull();
	afx_msg void OnSelchangeTransportCombo();
	afx_msg void OnTransportsExit();
	afx_msg void OnSelchangeVeterancy();
	afx_msg void OnSelchangeWaypointCombo();
	virtual BOOL OnInitDialog();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_TEAMREINFORCEMENT_H__29B5C0C7_10E8_4869_8B43_815422C51C24__INCLUDED_)
