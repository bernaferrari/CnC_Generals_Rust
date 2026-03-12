#if !defined(AFX_CELLWIDTH_H__E4023BA4_28DE_11D5_8CE0_00010297BBAC__INCLUDED_)
#define AFX_CELLWIDTH_H__E4023BA4_28DE_11D5_8CE0_00010297BBAC__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// CellWidth.h : header file
//

/////////////////////////////////////////////////////////////////////////////
/// CellWidth modal dialog to set the cell width.

class CellWidth : public CDialog
{
// Construction
public:
	CellWidth(int cellWidth, CWnd* pParent = NULL);   // standard constructor

	int GetCellWidth(void) {return mCellWidth;};
// Dialog Data
	//{{AFX_DATA(CellWidth)
	enum { IDD = IDD_CellWidth };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(CellWidth)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	virtual void OnOK();
	//}}AFX_VIRTUAL

// Implementation
protected:
	int mCellWidth;

protected:

	// Generated message map functions
	//{{AFX_MSG(CellWidth)
	virtual BOOL OnInitDialog();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_CELLWIDTH_H__E4023BA4_28DE_11D5_8CE0_00010297BBAC__INCLUDED_)
