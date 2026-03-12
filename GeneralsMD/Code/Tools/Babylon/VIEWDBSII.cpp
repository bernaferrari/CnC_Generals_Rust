// VIEWDBSII.cpp : implementation file
//

#include "stdafx.h"
#include "Babylon.h"
#include "VIEWDBSII.h"

#ifdef _DEBUG
#define new DEBUG_NEW
#undef THIS_FILE
static char THIS_FILE[] = __FILE__;
#endif

/////////////////////////////////////////////////////////////////////////////
// VIEWDBSII dialog


VIEWDBSII::VIEWDBSII(CWnd* pParent /*=NULL*/)
	: CDialog(VIEWDBSII::IDD, pParent)
{
	//{{AFX_DATA_INIT(VIEWDBSII)
		// NOTE: the ClassWizard will add member initialization here
	//}}AFX_DATA_INIT
}


void VIEWDBSII::DoDataExchange(CDataExchange* pDX)
{
	CDialog::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(VIEWDBSII)
		// NOTE: the ClassWizard will add DDX and DDV calls here
	//}}AFX_DATA_MAP
}


BEGIN_MESSAGE_MAP(VIEWDBSII, CDialog)
	//{{AFX_MSG_MAP(VIEWDBSII)
		// NOTE: the ClassWizard will add message map macros here
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// VIEWDBSII message handlers
