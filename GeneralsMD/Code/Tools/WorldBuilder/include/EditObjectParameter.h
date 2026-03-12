#if !defined(AFX_EDITOBJPARAMETER_H__465E4002_6405_47E3_97BA_D46A8C108600__INCLUDED_)
#define AFX_EDITOBJPARAMETER_H__465E4002_6405_47E3_97BA_D46A8C108600__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// EditObjectParameter.h : header file
//
#include "GameLogic/Scripts.h"
class SidesList;
/////////////////////////////////////////////////////////////////////////////
// EditObjectParameter dialog

class EditObjectParameter : public CDialog
{
friend class EditParameter;
// Construction
public:
	EditObjectParameter(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(EditObjectParameter)
	enum { IDD = IDD_EDIT_OBJECT_PARAMETER };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(EditObjectParameter)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	virtual BOOL OnNotify(WPARAM wParam, LPARAM lParam, LRESULT* pResult);
	//}}AFX_VIRTUAL

// Implementation


protected:
	HTREEITEM findOrAdd(HTREEITEM parent, const char *pLabel);
	void addObject( const ThingTemplate *thingTemplate  );
	void addObjectLists( );

protected:
	Parameter		*m_parameter;
	CTreeCtrl					m_objectTreeView;

protected:

	// Generated message map functions
	//{{AFX_MSG(EditObjectParameter)
	virtual BOOL OnInitDialog();
	virtual void OnOK();
	virtual void OnCancel();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_EDITOBJPARAMETER_H__465E4002_6405_47E3_97BA_D46A8C108600__INCLUDED_)
