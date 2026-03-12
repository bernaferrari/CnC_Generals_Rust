// Download.h : Declaration of the CDownload

#ifndef __DOWNLOAD_H_
#define __DOWNLOAD_H_

//#include "../resource.h"       // main symbols
#include "WWDownload/ftp.h"
#include "WWDownload/downloaddefs.h"

/////////////////////////////////////////////////////////////////////////////
// CDownload

class IDownload
{
 public:
	virtual HRESULT OnError( int error )=0;
	virtual HRESULT OnEnd()=0;
	virtual HRESULT OnQueryResume()=0;
	virtual HRESULT OnProgressUpdate(int bytesread, int totalsize, int timetaken, int timeleft)=0;
	virtual HRESULT OnStatusUpdate(int status)=0;
};

class CDownload
{
public:
	CDownload(IDownload *listener) :
		Listener(listener)
	{
		m_Status			= DOWNLOADSTATUS_NONE;
		m_TimeStarted		= 0;
		m_StartPosition		= 0;
		m_FileSize			= 0;
		m_BytesRead			= 0;
		m_Server[ 0 ]		= '\0';
		m_Login[ 0 ]		= '\0';
		m_Password[ 0 ]		= '\0';
		m_File[ 0 ]			= '\0';
		m_LocalFile[ 0 ]	= '\0';
		m_RegKey[ 0 ]		= '\0';
		m_Ftp				= new( Cftp );
		m_TryResume = false;

		m_predictions = 0;
		for (int i=0; i<8; ++i)
		{
			m_predictionTimes[i] = 0;
		}
	}
	~CDownload() { delete m_Ftp; }

public:
	virtual HRESULT PumpMessages();
	virtual HRESULT Abort();
	virtual HRESULT DownloadFile(LPCSTR server, LPCSTR username, LPCSTR password, LPCSTR file, LPCSTR localfile, LPCSTR regkey, bool tryresume=true);
	virtual HRESULT GetLastLocalFile(char *local_file, int maxlen);

private:
	char m_Server[ 256 ];
	char m_Login[ 64 ];
	char m_Password[ 64 ];
	char m_File[ 256 ];
	char m_LocalFile[ 256 ];

	char m_LastLocalFile[ 256 ];

	char m_RegKey[ 256 ];
	int  m_Status;
	int  m_TimeStarted;
	int  m_StartPosition;
	int  m_FileSize;
	int  m_BytesRead;
	bool m_TryResume;

	int m_predictions;
	int m_predictionTimes[8];

	Cftp			*m_Ftp;
	IDownload	*Listener;
};

#endif //__DOWNLOAD_H_
