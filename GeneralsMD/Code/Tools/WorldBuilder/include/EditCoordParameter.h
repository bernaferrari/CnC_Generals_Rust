#if !defined(AFX_EDITCOORDPARAMETER_H__465E4002_6405_47E3_97BA_D46A8C108600__INCLUDED_)
#define AFX_EDITCOORDPARAMETER_H__465E4002_6405_47E3_97BA_D46A8C108600__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// EditCoordParameter.h : header file
//
#include "GameLogic/Scripts.h"
class SidesList;
/////////////////////////////////////////////////////////////////////////////
// EditCoordParameter dialog

class EditCoordParameter : public CDialog
{
friend class EditParameter;
// Construction
public:
	EditCoordParameter(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(EditCoordParameter)
	enum { IDD = IDD_EDIT_COORD_PARAMETER };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(EditCoordParameter)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation


protected:

protected:
	Parameter		*m_parameter;
	Coord3D			 m_coord;

protected:

	// Generated message map functions
	//{{AFX_MSG(EditCoordParameter)
	virtual BOOL OnInitDialog();
	virtual void OnOK();
	virtual void OnCancel();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_EDITCOORDPARAMETER_H__465E4002_6405_47E3_97BA_D46A8C108600__INCLUDED_)
