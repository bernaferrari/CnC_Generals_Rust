// ProceedDlg.cpp : implementation file
//

#include "stdafx.h"
#include "Babylon.h"
#include "ProceedDlg.h"

#ifdef _DEBUG
#define new DEBUG_NEW
#undef THIS_FILE
static char THIS_FILE[] = __FILE__;
#endif

/////////////////////////////////////////////////////////////////////////////
// ProceedDlg dialog


ProceedDlg::ProceedDlg(const char *nmessage, CWnd* pParent /*=NULL*/)
	: CDialog(ProceedDlg::IDD, pParent)
{
	//{{AFX_DATA_INIT(ProceedDlg)
		// NOTE: the ClassWizard will add member initialization here
	//}}AFX_DATA_INIT
	message = nmessage;
}


void ProceedDlg::DoDataExchange(CDataExchange* pDX)
{
	CDialog::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(ProceedDlg)
		// NOTE: the ClassWizard will add DDX and DDV calls here
	//}}AFX_DATA_MAP
}


BEGIN_MESSAGE_MAP(ProceedDlg, CDialog)
	//{{AFX_MSG_MAP(ProceedDlg)
	ON_BN_CLICKED(IDC_YES, OnYes)
	ON_BN_CLICKED(IDC_ALWAYS, OnAlways)
	ON_BN_CLICKED(IDC_NO, OnNo)
	ON_WM_CLOSE()
	ON_BN_CLICKED(IDC_BUTTON_NO, OnNo)
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// ProceedDlg message handlers

void ProceedDlg::OnYes() 
{
	// TODO: Add your control notification handler code here
	EndDialog ( IDYES );
	
}								 

void ProceedDlg::OnAlways() 
{
	// TODO: Add your control notification handler code here
	EndDialog ( IDALWAYS );
	
}

void ProceedDlg::OnNo() 
{
	// TODO: Add your control notification handler code here
	EndDialog ( IDNO );
	
}

void ProceedDlg::OnClose() 
{
	// TODO: Add your message handler code here and/or call default
	
	EndDialog ( IDNO );
	CDialog::OnClose();
}

BOOL ProceedDlg::OnInitDialog() 
{
	CDialog::OnInitDialog();
	
	SetDlgItemText ( IDC_MESSAGE, message );	
	
	return TRUE;  // return TRUE unless you set the focus to a control
	              // EXCEPTION: OCX Property Pages should return FALSE
}
