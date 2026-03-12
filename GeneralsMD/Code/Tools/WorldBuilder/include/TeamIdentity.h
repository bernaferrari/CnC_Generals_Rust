#if !defined(AFX_TEAMIDENTITY_H__B37DB2FD_297F_4D0E_AACD_193BE30C6C85__INCLUDED_)
#define AFX_TEAMIDENTITY_H__B37DB2FD_297F_4D0E_AACD_193BE30C6C85__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// TeamIdentity.h : header file
//
class Dict;
class SidesList;

/////////////////////////////////////////////////////////////////////////////
// TeamIdentity dialog

class TeamIdentity : public CPropertyPage
{
// Construction
public:
	TeamIdentity();   // standard constructor

// Dialog Data
	//{{AFX_DATA(TeamIdentity)
	enum { IDD = IDD_TeamIdentity };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(TeamIdentity)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	virtual BOOL OnCommand(WPARAM wParam, LPARAM lParam);
	//}}AFX_VIRTUAL

protected:
	Dict		*m_teamDict;
	SidesList	*m_sides;
protected:
	void loadUnitsInfo(int idcMinUnit, NameKeyType keyMinUnit, 
								int idcMaxUnit, NameKeyType keyMaxUnit,
								int idcUnitType, NameKeyType keyUnitType);
	void OnUnitTypeButton(Int idcUnitType);

public:
	void setTeamDict(Dict *pDict) {m_teamDict = pDict;};
	void setSidesList(SidesList *pSides) {m_sides = pSides;};
// Implementation
protected:

	// Generated message map functions
	//{{AFX_MSG(TeamIdentity)
	virtual BOOL OnInitDialog();
	afx_msg void OnAiRecruitable();
	afx_msg void OnAutoReinforce();
	afx_msg void OnBaseDefense();
	afx_msg void OnChangeDescription();
	afx_msg void OnChangeMax();
	afx_msg void OnChangePriorityDecrease();
	afx_msg void OnChangePriorityIncrease();
	afx_msg void OnSelchangeProductionCondition();
	afx_msg void OnChangeProductionPriority();
	afx_msg void OnSelchangeHomeWaypoint();
	afx_msg void OnUnitType1Button();
	afx_msg void OnUnitType2Button();
	afx_msg void OnUnitType3Button();
	afx_msg void OnUnitType4Button();
	afx_msg void OnUnitType5Button();
	afx_msg void OnUnitType6Button();
	afx_msg void OnUnitType7Button();
	afx_msg void OnExecuteActions();
	afx_msg void OnChangeTeamName();
	afx_msg void OnTeamSingleton();
	afx_msg void OnKillfocusTeamName();
	afx_msg void OnSelendokTeamowner();
	afx_msg void OnChangeTeamBuildFrames();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_TEAMIDENTITY_H__B37DB2FD_297F_4D0E_AACD_193BE30C6C85__INCLUDED_)
