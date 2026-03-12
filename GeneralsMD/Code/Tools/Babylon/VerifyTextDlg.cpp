// VerifyTextDlg.cpp : implementation file
//

#include "stdafx.h"
#include "Babylon.h"
#include "VerifyTextDlg.h"

#ifdef _DEBUG
#define new DEBUG_NEW
#undef THIS_FILE
static char THIS_FILE[] = __FILE__;
#endif

/////////////////////////////////////////////////////////////////////////////
// CVerifyTextDlg dialog


CVerifyTextDlg::CVerifyTextDlg( char *trans, char *orig, CWnd* pParent /*=NULL*/)
	: CDialog(CVerifyTextDlg::IDD, pParent)
{
	//{{AFX_DATA_INIT(CVerifyTextDlg)
		// NOTE: the ClassWizard will add member initialization here
	//}}AFX_DATA_INIT

	m_trans = trans;
	m_orig = orig;
}


void CVerifyTextDlg::DoDataExchange(CDataExchange* pDX)
{
	CDialog::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(CVerifyTextDlg)
		// NOTE: the ClassWizard will add DDX and DDV calls here
	//}}AFX_DATA_MAP
}


BEGIN_MESSAGE_MAP(CVerifyTextDlg, CDialog)
	//{{AFX_MSG_MAP(CVerifyTextDlg)
	ON_BN_CLICKED(IDC_NOMATCH, OnNomatch)
	ON_BN_CLICKED(IDC_MATCH, OnMatch)
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// CVerifyTextDlg message handlers

void CVerifyTextDlg::OnNomatch() 
{

	EndDialog ( IDNO );
	
}

void CVerifyTextDlg::OnMatch() 
{

	EndDialog ( IDYES );
	
}

BOOL CVerifyTextDlg::OnInitDialog() 
{
	CDialog::OnInitDialog();

	SetDlgItemText ( IDC_TRANS, m_trans );
	SetDlgItemText ( IDC_ORIG, m_orig );
		
	
	return TRUE;  // return TRUE unless you set the focus to a control
	              // EXCEPTION: OCX Property Pages should return FALSE
}
