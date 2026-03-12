#include "StdAfx.h"
#include "Resource.h"
#include "CSwitchesDialog.h"

#include "ParticleEditorDialog.h"

CSwitchesDialog::CSwitchesDialog(UINT nIDTemplate, CWnd* pParentWnd) : CDialog(nIDTemplate, pParentWnd)
{

}

void CSwitchesDialog::InitPanel( void )
{

}

// if true, updates the UI from the Particle System. 
// if false, updates the Particle System from the UI
void CSwitchesDialog::performUpdate( IN Bool toUI )
{
	DebugWindowDialog *parent = GetDWDParent();
	if (!parent) {
		return;
	}

	{ // update hollowness
		CButton *pWnd;
		pWnd = (CButton*)GetDlgItem(IDC_PSEd_Hollow);
		if (pWnd) {
			Bool hollow;
			if (toUI) {
				parent->getSwitchFromSystem(ST_HOLLOW, hollow);
				pWnd->SetCheck(hollow);
			} else {
				hollow = pWnd->GetCheck();
				parent->updateSwitchToSystem(ST_HOLLOW, hollow);
			}
		}
	}

	{ // update one shot
		CButton *pWnd;
		pWnd = (CButton*)GetDlgItem(IDC_PSEd_OneShot);
		if (pWnd) {
			Bool oneShot;
			if (toUI) {
				parent->getSwitchFromSystem(ST_ONESHOT, oneShot);
				pWnd->SetCheck(oneShot);
			} else {
				oneShot = pWnd->GetCheck();
				parent->updateSwitchToSystem(ST_ONESHOT, oneShot);
			}
		}
	}

	{ // update Ground Aligned
		CButton *pWnd;
		pWnd = (CButton*)GetDlgItem(IDC_PSEd_GroundAligned);
		if (pWnd) {
			Bool groundAlign;
			if (toUI) {
				parent->getSwitchFromSystem(ST_ALIGNXY, groundAlign);
				pWnd->SetCheck(groundAlign);
			} else {
				groundAlign = pWnd->GetCheck();
				parent->updateSwitchToSystem(ST_ALIGNXY, groundAlign);
			}
		}
	}

	{ // update Emit above ground only
		CButton *pWnd;
		pWnd = (CButton*)GetDlgItem(IDC_PSEd_EmitAboveGroundOnly);
		if (pWnd) {
			Bool aboveGroundOnly;
			if (toUI) {
				parent->getSwitchFromSystem(ST_EMITABOVEGROUNDONLY, aboveGroundOnly);
				pWnd->SetCheck(aboveGroundOnly);
			} else {
				aboveGroundOnly = pWnd->GetCheck();
				parent->updateSwitchToSystem(ST_EMITABOVEGROUNDONLY, aboveGroundOnly);
			}
		}
	}

	{ // update Particle Up towards emitter
		CButton *pWnd;
		pWnd = (CButton*)GetDlgItem(IDC_PSEd_ParticleUpTowardsEmitter);
		if (pWnd) {
			Bool upTowardsEmitter;
			if (toUI) {
				parent->getSwitchFromSystem(ST_PARTICLEUPTOWARDSEMITTER, upTowardsEmitter);
				pWnd->SetCheck(upTowardsEmitter);
			} else {
				upTowardsEmitter = pWnd->GetCheck();
				parent->updateSwitchToSystem(ST_PARTICLEUPTOWARDSEMITTER, upTowardsEmitter);
			}
		}
	}
}

void CSwitchesDialog::OnParticleSystemEdit()
{
	DebugWindowDialog *pParent = GetDWDParent();
	if (!pParent) {
		return;
	}
	
	pParent->signalParticleSystemEdit();
}

BEGIN_MESSAGE_MAP(CSwitchesDialog, CDialog)
	ON_BN_CLICKED(IDC_PSEd_OneShot, OnParticleSystemEdit)
	ON_BN_CLICKED(IDC_PSEd_Hollow, OnParticleSystemEdit)
	ON_BN_CLICKED(IDC_PSEd_GroundAligned, OnParticleSystemEdit)
	ON_BN_CLICKED(IDC_PSEd_EmitAboveGroundOnly, OnParticleSystemEdit)
	ON_BN_CLICKED(IDC_PSEd_ParticleUpTowardsEmitter, OnParticleSystemEdit)
END_MESSAGE_MAP()
