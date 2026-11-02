**K-openMapper <버전>** - 맵 편집기 [openMapper](https://github.com/openOMSI-org/openMapper)에 K-openMapper 수정을 얹은 비공식 빌드입니다. 앞 숫자는 바탕이 된 openMapper 버전, `-k` 뒤 숫자는 그 위의 K-openMapper 빌드 번호입니다. openMapper 공식 프로젝트와 관계없는 독립 포크이며, 문제는 이 저장소의 이슈로 알려 주세요. 라이선스는 원본과 같은 AGPL-3.0-only입니다.

K-OMSI 0.4.10까지는 K-OMSI 묶음 안 `openMapper` 폴더에 들어 있었고, 이제는 이 저장소에서 따로 내려받습니다. 이 빌드의 자동 업데이트는 공식 openMapper가 아니라 K-openMapper 릴리스를 봅니다.

## 공식 openMapper에 더한 것
- **보기 > 노선 표시**: 지도 위에 노선(TTData)을 그리고, 표시할 노선을 하나만 고를 수 있습니다. 차로를 바꾸는 구간은 S자 곡선으로 그립니다.
- **파일 > 노선도 내보내기**: 지도 도로망 위 또는 노선만, SVG와 PNG로 저장합니다.
- **그리기 수정**: 주차칸처럼 도로 위에 납작하게 놓인 오브젝트가 도로에 묻히던 문제, 낮 텍스처가 검정이고 야간 텍스처만 있는 오브젝트(정류장 도움 화살표 등)가 검은 원으로 보이던 문제를 고쳤습니다.
- 새 문구의 한국어 번역, 화면 캡처 실행 때 자동 저장을 남기지 않음.

## 내려받기
Windows는 `openMapper-<버전>-windows-x64.zip`(압축 풀고 `openmapper.exe`) 또는 `-setup.exe`(설치판). Linux·macOS 묶음도 함께 올라갑니다.
