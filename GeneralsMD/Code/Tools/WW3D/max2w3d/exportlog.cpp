#include "exportlog.h"
#include "logdlg.h"
#include <assert.h>


/*
** Static variables
*/
LogDataDialogClass * _LogDialog = NULL;


/*
**
** ExportLog implementation.  Note, this is a class which only contains static functions.
**
*/


/***********************************************************************************************
 * ExportLog::Init -- Initialize the export logging system                                     *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   10/30/2000 gth : Created.                                                                 *
 *=============================================================================================*/
void ExportLog::Init(HWND parent)
{
	assert(_LogDialog == NULL);
	_LogDialog = new LogDataDialogClass(parent);
}


/***********************************************************************************************
 * ExportLog::Shutdown -- Shutdown the export logging system                                   *
 *                                                                                             *
 * INPUT:                                                                                      *
 * wait_for_ok - should we wait for the user to press OK on the dialog?                        *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   10/30/2000 gth : Created.                                                                 *
 *=============================================================================================*/
void ExportLog::Shutdown(bool wait_for_ok)
{
	if (_LogDialog != NULL) {

		if (wait_for_ok) {
			_LogDialog->Wait_OK();
		}

		delete _LogDialog;
		_LogDialog = NULL;
	}
}


/***********************************************************************************************
 * ExportLog::printf -- Print a string to the log window                                       *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   10/30/2000 gth : Created.                                                                 *
 *=============================================================================================*/
void ExportLog::printf(char * format, ...)
{
	if (_LogDialog != NULL) {
		va_list arguments;
		va_start(arguments, format);
		_LogDialog->printf(format,arguments);
	}
}


/***********************************************************************************************
 * ExportLog::rprintf -- Print a string over the last line printed                             *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   10/30/2000 gth : Created.                                                                 *
 *=============================================================================================*/
void ExportLog::rprintf(char * format, ...)
{
	if (_LogDialog != NULL) {
		va_list arguments;
		va_start(arguments, format);
		_LogDialog->rprintf(format,arguments);
	}
}


/***********************************************************************************************
 * ExportLog::updatebar -- Set the position of the progress bar                                *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   10/30/2000 gth : Created.                                                                 *
 *=============================================================================================*/
void ExportLog::updatebar(float position, float total)
{
	if (_LogDialog != NULL) {
		_LogDialog->updatebar(position,total);
	}
}


