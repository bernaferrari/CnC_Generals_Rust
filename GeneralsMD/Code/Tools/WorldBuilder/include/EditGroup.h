#if !defined(AFX_EDITGROUP_H__712F9978_4300_4625_9364_39E903FA3284__INCLUDED_)
#define AFX_EDITGROUP_H__712F9978_4300_4625_9364_39E903FA3284__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// EditGroup.h : header file
//

class ScriptGroup;
/////////////////////////////////////////////////////////////////////////////
// EditGroup dialog

class EditGroup : public CDialog
{
// Construction
public:
	EditGroup(ScriptGroup *pGroup, CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(EditGroup)
	enum { IDD = IDD_EDIT_GROUP };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(EditGroup)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
	protected:
		ScriptGroup *m_scriptGroup;

protected:

	// Generated message map functions
	//{{AFX_MSG(EditGroup)
	virtual void OnOK();
	virtual BOOL OnInitDialog();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_EDITGROUP_H__712F9978_4300_4625_9364_39E903FA3284__INCLUDED_)
