// CellWidth.cpp : implementation file
//

#include "stdafx.h"
#include "worldbuilder.h"
#include "CellWidth.h"

/////////////////////////////////////////////////////////////////////////////
// CellWidth dialog

/// Constructor and set initial cell width.
CellWidth::CellWidth(int cellWidth, CWnd* pParent /*=NULL*/)
	: CDialog(CellWidth::IDD, pParent),
	mCellWidth(cellWidth)
{
	//{{AFX_DATA_INIT(CellWidth)
		// NOTE: the ClassWizard will add member initialization here
	//}}AFX_DATA_INIT
}


void CellWidth::DoDataExchange(CDataExchange* pDX)
{
	CDialog::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(CellWidth)
		// NOTE: the ClassWizard will add DDX and DDV calls here
	//}}AFX_DATA_MAP
}


/////////////////////////////////////////////////////////////////////////////
// CellWidth message handlers

/// Get the cell width value from the ui on ok.
void CellWidth::OnOK() 
{
	CWnd *combo = GetDlgItem(IDC_CELL_WIDTH);
	CString val;
	if (combo) {
		combo->GetWindowText(val);
		mCellWidth = atoi(val);
	}
	CDialog::OnOK();
}


/// Set the initial value of cell width into the combobox.
BOOL CellWidth::OnInitDialog() 
{
	CDialog::OnInitDialog();
	
	CWnd *combo = GetDlgItem(IDC_CELL_WIDTH);
	CString val;
	val.Format("%d", mCellWidth);
	if (combo) combo->SetWindowText(val);
	
	return TRUE;  // return TRUE unless you set the focus to a control
	              // EXCEPTION: OCX Property Pages should return FALSE
}

BEGIN_MESSAGE_MAP(CellWidth, CDialog)
	//{{AFX_MSG_MAP(CellWidth)
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

