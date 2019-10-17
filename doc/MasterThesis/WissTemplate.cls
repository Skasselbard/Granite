\ProvidesClass{WissTemplate}
\DeclareOption{ngerman}{
	\PassOptionsToPackage{\CurrentOption}{babel}
	}
\DeclareOption{english}{
	\PassOptionsToPackage{\CurrentOption}{babel}
	}
\DeclareOption{BCOR}{
	\newcommand{\BCOR}{}
	}
\DeclareOption*{\PassOptionsToClass{\CurrentOption}{scrbook}}
\ProcessOptions\relax
\LoadClass[12pt,a4paper,headsepline,bibliography=totoc,listof=totoc,oneside,fleqn,numbers=noenddot]{scrbook}



\RequirePackage[utf8]{inputenc}
%\usepackage[T1]{fontenc}
\RequirePackage{lmodern}
\RequirePackage{babel}%german
%\usepackage[english]{babel}%english
\RequirePackage{amsmath,amssymb,amsthm}
\RequirePackage{graphicx}
\RequirePackage{xcolor}     %für Farbe
\RequirePackage{empheq}     %für Kästen um Formel ohne die Nummerierung mit im Rahmen zu haben ansonsten Paket framed angucken
\RequirePackage{eurosym}
\RequirePackage{listings}   %Für code-Listings
\RequirePackage{pbox}
%\usepackage[]{algorithm2e} %Pseudocode
\RequirePackage{algpseudocode}
\RequirePackage[section]{algorithm}
\RequirePackage{tikz} %Zum "malen" in latex
\floatname{algorithm}{Algorithmus}
%PDF Lesezeichen
\RequirePackage[
bookmarksopen=true,
colorlinks=true,
linkcolor=black,
anchorcolor=black,
citecolor=black,
filecolor=black,
menucolor=black,
urlcolor=black,
plainpages=false,
hypertexnames=false]{hyperref}

\RequirePackage[automark]{scrpage2}
\RequirePackage{nomencl} %Abkürzungen

\RequirePackage{wallpaper}
\RequirePackage{tcolorbox}
\RequirePackage{layouts}
\RequirePackage{tocloft}
%Z Notation (see: http://fmt.cs.utwente.nl/courses/fmse/software/refcard.pdf for reference)
%\usepackage{zed-csp}
%für \llangle (doppeltes \langle)
%\usepackage{MnSymbol}

\RequirePackage{subcaption} %Mehrere Abbildungen in einer Abbildung
\RequirePackage{blindtext} %lorem ipsum
\RequirePackage{booktabs} %schönere Tabellen
\RequirePackage{multirow} %Mehrere Zeilen einer Tabelle zusammenführen

\newtheoremstyle{mydefinitionstyle}{}{}{\itshape}{}{\bfseries}{:}{ }{\thmname{#1}\thmnumber{ #2}\thmnote{ (#3)}} %Für Definitionen
\newtheoremstyle{myexamplestyle}{}{}{}{}{\bfseries}{:}{ }{\thmname{#1}\thmnumber{ #2}\thmnote{ (#3)}} %Für Beispiele

\theoremstyle{mydefinitionstyle}
\newtheorem{definition}{Definition}[section]
\theoremstyle{myexamplestyle}

\iflanguage{english}{
	\newtheorem{exmp}{Example}[section]
}
{
	\newtheorem{exmp}{Beispiel}[section]
}

\iflanguage{german}{
	\renewcommand{\algorithmicrequire}{\textbf{Eingabe:}}
	\renewcommand{\algorithmicensure}{\textbf{Ausgabe:}}
}
%\renewcommand\thetheorem{\thechapter.\thesection.\arabic{exmp}}
%	BINDUNGSKOOREKTUR 
%\newcommand{\BCOR}{} %Für Bindungskorrektur auskommentieren
\ifdefined\BCOR
%Bindungskorrektur von ca. 1cm bei Hardcoverbindung
\RequirePackage[left=1.233in,right=0.833in,top=0.97in,bottom=1.75in, includehead]{geometry}
\else
%Keine Bindungskorrektur
\RequirePackage[left=1.033in,right=1.033in,top=0.97in,bottom=1.75in, includehead]{geometry}
\fi
% % % % % % % % % % % % % %
%
%	NEW COMMANDS
%
% % % % % % % % % % % % % %


\newcommand{\ol}{\overline}
\newcommand{\name}{VORNAME NACHNAME}  %Name für Titelseite und Erklärung
\newcommand{\strasse}{STRASSE}
\newcommand{\stadt}{PLZ ORT}
\newcommand{\matrikel}{Matrikel-Nr.: 123456789}
\newcommand{\course}{Informationstechnologie / Technische Informatik}
\newcommand{\betreuer}{Prof. Dr.-Ing. Max Mustermann} 
\newcommand{\Titel}{Template für eine Studienarbeit}
\newcommand{\Type}{Masterarbeit} %Bachelorarbeit, Projektarbeit, Literaturarbeit, etc.

\newcommand{\bigand}{\bigwedge}
\newcommand{\bigor}{\bigvee}
\newcommand{\engl}[1]{{(engl. \textit{#1})}}
% % % % % % % % % % % % % %
%
%	SETTINGS
%
% % % % % % % % % % % % % %

\let\abk\nomenclature
% Deutsche Überschrift
\iflanguage{english}{
	\renewcommand{\nomname}{List of Abbreviations}
}
{
	\renewcommand{\nomname}{Abkürzungsverzeichnis}
}
% Abstand zw. Abkürzung und Bedeutung
\setlength{\nomlabelwidth}{.30\textwidth}
% Punkte zw. Abkürzung und Erklärung
\renewcommand{\nomlabel}[1]{#1 \dotfill}
\setlength{\nomitemsep}{-\parsep}
\makenomenclature

\setlength{\cftfigindent}{0em}
\setlength{\cfttabindent}{0em}

\captionsetup[subfigure]{labelformat=simple}
\renewcommand\thesubfigure{(\alph{subfigure})} %Subfigures vernünftig in crossreferenzen anzeigen

%GRAFIK PFAD
%Pfade für Grafiken, mehrere Pfade können hinzugefügt werden 
\graphicspath{{./Figures/}}

%listing settings
\definecolor{mygreen}{rgb}{0,0.56,0}
\definecolor{mygray}{rgb}{0.5,0.5,0.5}
\definecolor{mymauve}{rgb}{0.58,0,0.82}

\lstset{ %
	backgroundcolor=\color{white},   % choose the background color; you must add \usepackage{color} or \usepackage{xcolor}
	basicstyle=\footnotesize,        % the size of the fonts that are used for the code
	breakatwhitespace=false,         % sets if automatic breaks should only happen at whitespace
	breaklines=true,                 % sets automatic line breaking
	captionpos=b,                    % sets the caption-position to bottom
	commentstyle=\color{mygreen},    % comment style
	deletekeywords={...},            % if you want to delete keywords from the given language
	escapeinside={\%*}{*)},          % if you want to add LaTeX within your code
	extendedchars=true,              % lets you use non-ASCII characters; for 8-bits encodings only, does not work with UTF-8
	frame=single,                    % adds a frame around the code
	keepspaces=true,                 % keeps spaces in text, useful for keeping indentation of code (possibly needs columns=flexible)
	keywordstyle=\bfseries\color{mymauve},       % keyword style
	language=Java,                 % the language of the code
	otherkeywords={String},            % if you want to add more keywords to the set
	numbers=left,                    % where to put the line-numbers; possible values are (none, left, right)
	numbersep=5pt,                   % how far the line-numbers are from the code
	numberstyle=\tiny\color{mygray}, % the style that is used for the line-numbers
	rulecolor=\color{black},         % if not set, the frame-color may be changed on line-breaks within not-black text (e.g. comments (green here))
	showspaces=false,                % show spaces everywhere adding particular underscores; it overrides 'showstringspaces'
	showstringspaces=false,          % underline spaces within strings only
	showtabs=false,                  % show tabs within strings adding particular underscores
	stepnumber=2,                    % the step between two line-numbers. If it's 1, each line will be numbered
	stringstyle=\color{blue},     % string literal style
	tabsize=2,                       % sets default tabsize to 2 spaces
	identifierstyle=\color{blue},
	title=\lstname                   % show the filename of files included with \lstinputlisting; also try caption instead of title
}

%Das ist das Uniblau
\definecolor{uniblau}{HTML}{004A99} 

%tcolorbox
\tcbuselibrary{skins}

\setkomafont{captionlabel}{\usekomafont{descriptionlabel}} 
\setkomafont{disposition}{\color{uniblau}\bfseries\sffamily}

\setcounter{secnumdepth}{3} %Nummerierungstiefe
\setcounter{tocdepth}{2} %Anzeige im Table of contents

\hypersetup{
	pdftitle   = {\Titel},
	pdfauthor  = {\name, \matrikel},
	pdfsubject = {\Type betreut von \betreuer}
}

\clearscrheadfoot % alten Standardkram raus
\ohead[\pagemark]{\pagemark} % oben rechts Seitenzahl laut Richtlinie
\ihead{\headmark} % automatischen Kapitelnamen rein

\pagestyle{scrheadings}