#if !defined(AFX_ScriptConditionsDlg_H__EEFDFF65_2440_4AFE_B5D6_9E887C8C2DED__INCLUDED_)
#define AFX_ScriptConditionsDlg_H__EEFDFF65_2440_4AFE_B5D6_9E887C8C2DED__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// ScriptConditionsDlg.h : header file
//
class Script;
class OrCondition;
class Condition;
class SidesList;
/////////////////////////////////////////////////////////////////////////////
// ScriptConditionsDlg dialog

class ScriptConditionsDlg : public CPropertyPage
{
	DECLARE_DYNCREATE(ScriptConditionsDlg)

// Construction
public:
	ScriptConditionsDlg();
	~ScriptConditionsDlg();

// Dialog Data
	//{{AFX_DATA(ScriptConditionsDlg)
	enum { IDD = IDD_ScriptConditions };
		// NOTE - ClassWizard will add data members here.
		//    DO NOT EDIT what you see in these blocks of generated code !
	//}}AFX_DATA


// Overrides
	// ClassWizard generate virtual function overrides
	//{{AFX_VIRTUAL(ScriptConditionsDlg)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
public:
	void setScript(Script *pScript) {m_script = pScript;}

protected:
	Script *m_script;	 // Doesn't change.
	OrCondition *m_orCondition; // Currently selected OR clause.
	Condition		*m_condition;		// Currently selected condition.
	Int					m_index; // Index of whatever is currently selected.

protected:
	void enableUI(void); 
	void loadList(void);
	Int doMoveUp( OrCondition **outWhichNow );
	Int doMoveDown( OrCondition **outWhichNow );
	void setSel(OrCondition *pOr, Condition *pCond);

protected:
	// Generated message map functions
	//{{AFX_MSG(ScriptConditionsDlg)
	virtual BOOL OnInitDialog();
	afx_msg void OnEditCondition();
	afx_msg void OnSelchangeConditionList();
	afx_msg void OnDblclkConditionList();
	afx_msg void OnOr();
	afx_msg void OnNew();
	afx_msg void OnDelete();
	afx_msg void OnCopy();
	afx_msg void OnMoveDown();
	afx_msg void OnMoveUp();
	afx_msg void OnChangeEditComment();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()

};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_ScriptConditionsDlg_H__EEFDFF65_2440_4AFE_B5D6_9E887C8C2DED__INCLUDED_)
